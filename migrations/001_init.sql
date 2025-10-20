CREATE TABLE IF NOT EXISTS results (
  run_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  shard INT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  schema_version INT NOT NULL,
  payload JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (run_id, task_id, shard)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_key ON results(idempotency_key);
