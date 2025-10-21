CREATE TABLE IF NOT EXISTS stast_results (
  id BIGSERIAL PRIMARY KEY,
  task TEXT NOT NULL,
  sample_id TEXT NOT NULL,
  qseqid TEXT NOT NULL,
  qlen BIGINT NOT NULL,
  sseqid TEXT NOT NULL,
  stitle TEXT NOT NULL,
  length BIGINT NOT NULL,
  pident DOUBLE PRECISION NOT NULL,
  evalue DOUBLE PRECISION NOT NULL,
  bitscore DOUBLE PRECISION NOT NULL,
  sscinames TEXT NOT NULL,
  staxids TEXT NOT NULL,
  rank TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_stast_sample_id ON stast_results(sample_id);

CREATE INDEX IF NOT EXISTS idx_stast_task ON stast_results(task);

CREATE INDEX IF NOT EXISTS idx_stast_staxids ON stast_results(staxids);

CREATE INDEX IF NOT EXISTS idx_stast_qseqid ON stast_results(qseqid);
