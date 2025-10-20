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

### GET /healthz

Returns `ok` if service is running.

## Development

```bash
cargo check
cargo test
cargo fmt
cargo clippy
```
