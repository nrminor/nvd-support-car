# NVD Support CAR

HTTP service for ingesting HTCondor cluster data into PostgreSQL.

## Features

- TLS termination (rustls) on configurable port
- Bearer token authentication
- Rate limiting (200 req/s, 400 burst)
- Gzip decompression of request bodies
- NDJSON parsing and validation
- Automatic database migrations
- Idempotent record insertion with conflict handling

## Setup

### Environment Variables

```bash
export DATABASE_URL=postgres://user:password@localhost/dbname
export INGEST_TOKEN=your-secret-token
export SERVER_PORT=443
export CERT_PATH=/path/to/cert.pem
export KEY_PATH=/path/to/key.pem
```

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

### GET /healthz

Returns `ok` if service is running.

## Development

```bash
cargo check
cargo test
cargo fmt
cargo clippy
```
