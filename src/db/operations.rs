use crate::error::AppError;
use crate::models::DummyRecord;

pub struct DbOperations;

impl DbOperations {
    pub async fn insert_record(
        db: &sqlx::PgPool,
        record: &DummyRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO results (run_id, task_id, shard, idempotency_key, schema_version, payload)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (idempotency_key) DO NOTHING",
        )
        .bind(&record.run_id)
        .bind(&record.task_id)
        .bind(record.shard)
        .bind(&record.idempotency_key)
        .bind(record.schema_version)
        .bind(&record.payload)
        .execute(db)
        .await
        .map_err(|e| AppError::InternalServerError(format!("database insert failed: {}", e)))?;

        Ok(())
    }

    pub async fn insert_records(
        db: &sqlx::PgPool,
        records: &[DummyRecord],
    ) -> Result<(), AppError> {
        for record in records {
            Self::insert_record(db, record).await?;
        }
        Ok(())
    }
}
