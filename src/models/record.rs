use serde::{Deserialize, Serialize};
use sqlx::{FromRow, postgres::PgArguments};

pub trait BulkInsertable: Sized {
    /// Number of fields that will be inserted
    fn field_count() -> usize;

    /// Table name for the INSERT statement
    fn table_name() -> &'static str;

    /// Comma-separated list of column names
    fn column_names() -> &'static str;

    /// Optional ON CONFLICT clause
    #[must_use]
    fn conflict_clause() -> Option<&'static str> {
        None
    }

    /// Bind this record's fields to the query
    fn bind_to(
        self,
        query: sqlx::query::Query<'_, sqlx::Postgres, PgArguments>,
    ) -> sqlx::query::Query<'_, sqlx::Postgres, PgArguments>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DummyRecord {
    pub run_id: String,
    pub task_id: String,
    pub shard: i32,
    pub idempotency_key: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct Gottcha2FullRecord {
    pub sample_id: String,    // extracted from filename
    pub level: String,        // LEVEL
    pub name: String,         // NAME
    pub taxid: String,        // TAXID
    pub read_count: i64,      // READ_COUNT
    pub total_bp_mapped: i64, // TOTAL_BP_MAPPED
    pub ani_ci95: f64,        // ANI_CI95
    pub covered_sig_len: i64, // COVERED_SIG_LEN
    pub best_sig_cov: f64,    // BEST_SIG_COV
    pub depth: f64,           // DEPTH
    pub rel_abundance: f64,   // REL_ABUNDANCE
}

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct StastRecord {
    pub task: String,
    pub sample_id: String, // renamed from 'sample' for consistency
    pub qseqid: String,
    pub qlen: i64,
    pub sseqid: String,
    pub stitle: String,
    pub length: i64,
    pub pident: f64,
    pub evalue: f64,
    pub bitscore: f64,
    pub sscinames: String,
    pub staxids: String,
    pub rank: String,
}

impl BulkInsertable for DummyRecord {
    fn field_count() -> usize {
        6
    }

    fn table_name() -> &'static str {
        "results"
    }

    fn column_names() -> &'static str {
        "run_id, task_id, shard, idempotency_key, schema_version, payload"
    }

    fn conflict_clause() -> Option<&'static str> {
        Some(" ON CONFLICT (idempotency_key) DO NOTHING")
    }

    fn bind_to(
        self,
        query: sqlx::query::Query<'_, sqlx::Postgres, PgArguments>,
    ) -> sqlx::query::Query<'_, sqlx::Postgres, PgArguments> {
        query
            .bind(self.run_id)
            .bind(self.task_id)
            .bind(self.shard)
            .bind(self.idempotency_key)
            .bind(self.schema_version)
            .bind(self.payload)
    }
}

impl BulkInsertable for Gottcha2FullRecord {
    fn field_count() -> usize {
        11
    }

    fn table_name() -> &'static str {
        "gottcha2_results"
    }

    fn column_names() -> &'static str {
        "sample_id, level, name, taxid, read_count, total_bp_mapped, ani_ci95, covered_sig_len, best_sig_cov, depth, rel_abundance"
    }

    fn bind_to(
        self,
        query: sqlx::query::Query<'_, sqlx::Postgres, PgArguments>,
    ) -> sqlx::query::Query<'_, sqlx::Postgres, PgArguments> {
        query
            .bind(self.sample_id)
            .bind(self.level)
            .bind(self.name)
            .bind(self.taxid)
            .bind(self.read_count)
            .bind(self.total_bp_mapped)
            .bind(self.ani_ci95)
            .bind(self.covered_sig_len)
            .bind(self.best_sig_cov)
            .bind(self.depth)
            .bind(self.rel_abundance)
    }
}

impl BulkInsertable for StastRecord {
    fn field_count() -> usize {
        13
    }

    fn table_name() -> &'static str {
        "stast_results"
    }

    fn column_names() -> &'static str {
        "task, sample_id, qseqid, qlen, sseqid, stitle, length, pident, evalue, bitscore, sscinames, staxids, rank"
    }

    fn bind_to(
        self,
        query: sqlx::query::Query<'_, sqlx::Postgres, PgArguments>,
    ) -> sqlx::query::Query<'_, sqlx::Postgres, PgArguments> {
        query
            .bind(self.task)
            .bind(self.sample_id)
            .bind(self.qseqid)
            .bind(self.qlen)
            .bind(self.sseqid)
            .bind(self.stitle)
            .bind(self.length)
            .bind(self.pident)
            .bind(self.evalue)
            .bind(self.bitscore)
            .bind(self.sscinames)
            .bind(self.staxids)
            .bind(self.rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_record_field_count_matches_columns() {
        let column_count = DummyRecord::column_names().split(", ").count();
        assert_eq!(
            DummyRecord::field_count(),
            column_count,
            "field_count() should match the number of columns"
        );
    }

    #[test]
    fn gottcha2_field_count_matches_columns() {
        let column_count = Gottcha2FullRecord::column_names().split(", ").count();
        assert_eq!(
            Gottcha2FullRecord::field_count(),
            column_count,
            "field_count() should match the number of columns"
        );
    }

    #[test]
    fn stast_field_count_matches_columns() {
        let column_count = StastRecord::column_names().split(", ").count();
        assert_eq!(
            StastRecord::field_count(),
            column_count,
            "field_count() should match the number of columns"
        );
    }

    #[test]
    fn dummy_record_table_name() {
        assert_eq!(DummyRecord::table_name(), "results");
    }

    #[test]
    fn gottcha2_table_name() {
        assert_eq!(Gottcha2FullRecord::table_name(), "gottcha2_results");
    }

    #[test]
    fn stast_table_name() {
        assert_eq!(StastRecord::table_name(), "stast_results");
    }

    #[test]
    fn dummy_record_has_conflict_clause() {
        assert!(
            DummyRecord::conflict_clause().is_some(),
            "DummyRecord should have a conflict clause"
        );
        assert!(
            DummyRecord::conflict_clause()
                .expect("DummyRecord should have a conflict clause")
                .contains("idempotency_key"),
            "Conflict clause should reference idempotency_key"
        );
    }

    #[test]
    fn gottcha2_and_stast_have_no_conflict_clause() {
        assert!(
            Gottcha2FullRecord::conflict_clause().is_none(),
            "Gottcha2FullRecord should not have a conflict clause"
        );
        assert!(
            StastRecord::conflict_clause().is_none(),
            "StastRecord should not have a conflict clause"
        );
    }
}
