CREATE TABLE IF NOT EXISTS gottcha2_results (
  id bigserial PRIMARY KEY,
  sample_id text NOT NULL,
  LEVEL text NOT NULL,
  name text NOT NULL,
  taxid text NOT NULL,
  read_count bigint NOT NULL,
  total_bp_mapped bigint NOT NULL,
  ani_ci95 double precision NOT NULL,
  covered_sig_len bigint NOT NULL,
  best_sig_cov double precision NOT NULL,
  depth double precision NOT NULL,
  rel_abundance double precision NOT NULL,
  created_at timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gottcha2_sample_id ON gottcha2_results(sample_id);

CREATE INDEX IF NOT EXISTS idx_gottcha2_level ON gottcha2_results(LEVEL);

CREATE INDEX IF NOT EXISTS idx_gottcha2_taxid ON gottcha2_results(taxid);
