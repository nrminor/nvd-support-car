# NVD "Support Car"

This repo is an experimental prototype for providing a dependency free
executable server that can operate as a "support car" for [NVD](). Its main
purpose at this stage is to handle authentication, receive data from HTCondor
nodes running NVD on parallel samples, and handle "upserting" those data into a
single, centralized dataset. It will gradually take on more responsibilities as
time goes on, but for now, its main objective is to centralize and simplify data
ingress and reduce many instances where data is put on disk into dozens of text
files.

It does a number of core backend service things, including:

- TLS termination on configurable port
- Bearer token authentication, where nodes simply need the expected token to
  form a connection with the support car
- NVD could theoretically run on an unbounded number of samples, so the support
  car comes with built-in rate limiting (200 req/s, 400 burst)
- data can come in big batches, so the support car expects it to be Gzip'd JSONL
  and handles decoding and deserializing it as such
- At startup, the support car handles database migrations as needed
- The support care also performs idempotent record insertion with conflict
  handling

## Setup

All configuration of the support care is currently managed through environment
variables, namely the following:

```bash
export DATABASE_URL=postgres://user:password@localhost/dbname
export INGEST_TOKEN=your-secret-token
export SERVER_PORT=443
export CERT_PATH=/path/to/cert.pem
export KEY_PATH=/path/to/key.pem
```

These variables will be read from the shell environment and used to configure
tha service at launch

### Running

```bash
cargo run --release
```

### Building for Other Platforms

```bash
cross build --target x86_64-unknown-linux-gnu --release
```

## API

### POST /ingest

Accepts gzipped NDJSON with bearer token authentication.

**Request:**

- Header: `Authorization: Bearer <token>`
- Body: gzipped NDJSON where each line is:
  ```json
  {
    "run_id": "string",
    "task_id": "string",
    "shard": 0,
    "idempotency_key": "unique-string",
    "schema_version": 1,
    "payload": {}
  }
  ```

**Response:** `200 OK` on success

### POST /ingest-gottcha2

Accepts gzipped NDJSON with GOTTCHA2 taxonomic abundance data.

**Request:**

- Header: `Authorization: Bearer <token>`
- Body: gzipped NDJSON where each line contains GOTTCHA2 fields

**Response:** `200 OK` on success

### POST /ingest-stast

Accepts gzipped NDJSON with STAST BLAST hit data.

**Request:**

- Header: `Authorization: Bearer <token>`
- Body: gzipped NDJSON where each line contains STAST fields

**Response:** `200 OK` on success

### GET /healthz

Returns `ok` if service is running.

## Development

### Adding New Data Ingestion Routes

To add a new data ingestion endpoint within this framework, follow these steps:

1. **Create a migration** in `migrations/00X_your_table.sql`:
   ```sql
   CREATE TABLE IF NOT EXISTS your_table (
     id BIGSERIAL PRIMARY KEY,
     field1 TEXT NOT NULL,
     field2 BIGINT NOT NULL,
     -- ... your fields
     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
   );
   
   CREATE INDEX IF NOT EXISTS idx_your_table_field1 ON your_table(field1);
   ```

2. **Create a model struct** in `src/models/record.rs`:
   ```rust
   #[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
   pub struct YourRecord {
       pub field1: String,
       pub field2: i64,
       // ... fields matching your table columns
   }
   ```

3. **Implement BulkInsertable** for your struct in `src/models/record.rs`:
   ```rust
   impl BulkInsertable for YourRecord {
       fn field_count() -> usize { 2 }  // number of fields
       
       fn table_name() -> &'static str { "your_table" }
       
       fn column_names() -> &'static str { "field1, field2" }
       
       fn bind_to<'q>(
           self,
           query: sqlx::query::Query<'q, sqlx::Postgres, PgArguments>,
       ) -> sqlx::query::Query<'q, sqlx::Postgres, PgArguments> {
           query
               .bind(self.field1)
               .bind(self.field2)
       }
   }
   ```

4. **Add batch insert function** in `src/db/operations.rs`:
   ```rust
   pub async fn batch_insert_your_type(
       rx: mpsc::Receiver<YourRecord>,
       db: &PgPool,
   ) -> Result<(), AppError> {
       batch_insert_from_channel(rx, db).await
   }
   ```

5. **Create handler** in `src/handlers/your_type.rs`:
   ```rust
   use axum::{body::Body, extract::State, http::{HeaderMap, StatusCode}, response::IntoResponse};
   use tokio::sync::mpsc;
   use crate::{
       db::operations::batch_insert_your_type,
       middleware::validate_bearer_token,
       services::parsing::parse_gzipped_jsonl,
       state::AppState,
   };
   
   pub async fn ingest_your_type(
       State(state): State<AppState>,
       headers: HeaderMap,
       body: Body,
   ) -> impl IntoResponse {
       if let Err(e) = validate_bearer_token(&state, &headers) {
           return e.into_response();
       }
   
       let (tx, rx) = mpsc::channel(1000);
       let parser = parse_gzipped_jsonl(body, tx);
       let inserter = batch_insert_your_type(rx, &state.db);
   
       if let Err(e) = tokio::try_join!(parser, inserter) {
           return e.into_response();
       }
   
       (StatusCode::OK, "ingested").into_response()
   }
   ```

6. **Wire it up**:
   - Export handler in `src/handlers/mod.rs`: `pub use your_type::ingest_your_type;`
   - Add route in `src/main.rs`: `.route("/ingest-your-type", post(ingest_your_type))`

The generic infrastructure handles all parsing, batching, and bulk SQL operations automatically.

### Testing

```bash
cargo check
cargo test
cargo fmt
cargo clippy
```
