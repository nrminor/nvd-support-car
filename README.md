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

## Installation

### Quick Install (Recommended)

Install the latest release using curl:

```bash
curl -fsSL https://raw.githubusercontent.com/nrminor/nvd-support-car/main/INSTALL.sh | bash
```

Or with wget:

```bash
wget -qO- https://raw.githubusercontent.com/nrminor/nvd-support-car/main/INSTALL.sh | bash
```

The installer will:

1. Download a pre-built binary for your platform (if available)
2. Fall back to building from source if no binary is available
3. Install to `~/.local/bin`
4. Provide setup instructions for the database and environment

### Install from Source with Cargo

If you prefer to build from source directly:

```bash
# Install from GitHub repository
cargo install --git https://github.com/nrminor/nvd-support-car.git
```

### Database Setup

After installation, set up the PostgreSQL database:

```bash
# Create database
psql -U postgres -c 'CREATE DATABASE nvd_support;'

# Run migrations
psql -U postgres -d nvd_support -f migrations/001_init.sql
psql -U postgres -d nvd_support -f migrations/002_gottcha2_full_table.sql
psql -U postgres -d nvd_support -f migrations/003_stast_table.sql
```

## Configuration

All configuration is managed through environment variables. See
[examples/.env.example](examples/.env.example) for a complete template.

```bash
export DATABASE_URL="postgresql://user:password@localhost/nvd_support"
export BEARER_TOKEN="your-secret-token"
export HOST="127.0.0.1"
export PORT="8080"

# Optional TLS configuration
export CERT_PATH="/path/to/cert.pem"
export KEY_PATH="/path/to/key.pem"
```

These variables will be read from the shell environment and used to configure
the service at launch.

## Running

After installation and configuration:

```bash
# If installed to PATH
nvd-support-car

# Or run directly
~/.local/bin/nvd-support-car

# For development
cargo run --release
```

## Building for Other Platforms

To build for different platforms:

```bash
# Using cross for Linux targets
cross build --target x86_64-unknown-linux-musl --release
cross build --target aarch64-unknown-linux-musl --release

# Native builds for macOS
cargo build --target x86_64-apple-darwin --release
cargo build --target aarch64-apple-darwin --release
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
   - Export handler in `src/handlers/mod.rs`:
     `pub use your_type::ingest_your_type;`
   - Add route in `src/main.rs`:
     `.route("/ingest-your-type", post(ingest_your_type))`

The generic infrastructure handles all parsing, batching, and bulk SQL
operations automatically.

### Testing

NVD Support Car uses a comprehensive three-tier testing strategy with 63 tests
covering unit, integration, and end-to-end scenarios.

#### Test Pyramid

```
        ▲
       /E2\      22 E2E Tests (~8s)
      /────\     Full stack: TLS + PostgreSQL + HTTP
     / Int  \    20 Integration Tests (~8s)  
    /  DB    \   Real PostgreSQL via testcontainers
   /───────── \  
  /  Unit      \ 21 Unit Tests (~9s)
 /   Tests      \ Fast, mock-based validation
/────────────────\
  Total: 63 tests (~25s)
```

#### Quick Start (No Docker)

Run fast unit tests with mock handlers:

```bash
# Using cargo
cargo test --lib
cargo test --test integration_test

# Using justfile
just test-unit
```

**Run time**: ~9 seconds\
**Requirements**: None (though infrastructure tests use testcontainers)

#### Full Test Suite (Docker Required)

```bash
# Run all tests
cargo test

# Or run test tiers separately
just test-unit          # Fast unit tests (~9s)
just test-integration   # Database integration (~8s)
just test-e2e          # Full E2E with TLS (~8s)
just test-all          # All tiers in sequence (~25s)
```

#### What's Tested

**Unit Tests** (`tests/integration_test.rs`):

- Health check endpoint
- Authentication (missing/invalid tokens)
- Data validation (malformed gzip, invalid JSON)
- Certificate generation
- Database helpers
- Server lifecycle

**Integration Tests** (`tests/integration_db_test.rs`):

- Batch insert for GOTTCHA2 records
- Batch insert for STAST records
- Concurrent database operations (10 parallel)
- Large dataset handling (1,000 records)
- Data integrity validation
- Table cleanup

**E2E Tests** (`tests/e2e_test.rs`):

- GOTTCHA2 ingestion over HTTPS
- STAST ingestion over HTTPS
- Authentication over TLS
- Concurrent client requests (10 parallel)
- TLS certificate validation
- Error handling (malformed data, empty payloads)
- Large payload handling (100 records)

#### Docker Requirement

Integration and E2E tests use [testcontainers](https://testcontainers.com/) to
spin up PostgreSQL containers automatically. Docker must be running:

```bash
# Check Docker is running
docker ps

# Start Docker if needed
# macOS: Open Docker Desktop
# Linux: sudo systemctl start docker
```

GitHub Actions CI handles Docker automatically - no manual setup required for
PRs.

#### Code Quality

```bash
cargo fmt              # Format code
cargo clippy           # Lint with strict rules
just check             # Format + clippy + test
```

#### Troubleshooting

**Port conflicts**: E2E tests use OS-assigned ports. Run sequentially if needed:

```bash
cargo test --test e2e_test -- --test-threads=1
```

**Leftover containers**: Tests clean up automatically, but if interrupted:

```bash
docker ps -a | grep postgres
docker rm -f $(docker ps -a -q --filter ancestor=postgres:16-alpine)
```

**Performance**: E2E tests are slower (~8s) due to TLS handshakes and container
startup. Use `just test-unit` for fast iteration during development.
