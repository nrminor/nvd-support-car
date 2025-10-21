use sqlx::PgPool;
use std::fmt::Write;
use tokio::sync::mpsc;

use crate::{
    error::AppError,
    models::record::{BulkInsertable, DummyRecord, Gottcha2FullRecord, StastRecord},
};

const BATCH_SIZE: usize = 500;

async fn bulk_insert_chunk<T: BulkInsertable>(
    db: &PgPool,
    records: Vec<T>, // Interior mutability alert: consumes records for binding
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }

    let field_count = T::field_count();

    // Build the SQL query with multiple VALUE rows
    let mut query = format!(
        "INSERT INTO {} ({}) VALUES ",
        T::table_name(),
        T::column_names()
    );

    // Add placeholders for each record
    for (i, _) in records.iter().enumerate() {
        if i > 0 {
            query.push_str(", ");
        }
        let offset = i * field_count;
        let placeholders: Vec<String> = (1..=field_count)
            .map(|j| format!("${}", offset + j))
            .collect();
        write!(&mut query, "({})", placeholders.join(", ")).expect("Failed to write to string");
    }

    // Add optional conflict clause
    if let Some(clause) = T::conflict_clause() {
        query.push_str(clause);
    }

    // Bind all records
    let mut q = sqlx::query(&query);
    for record in records {
        q = record.bind_to(q);
    }

    // Execute the bulk insert
    q.execute(db)
        .await
        .map_err(|e| AppError::InternalServerError(format!("bulk insert failed: {e}")))?;

    Ok(())
}

async fn insert_records<T: BulkInsertable>(
    db: &PgPool,
    mut records: Vec<T>, // Consumes the vector since we need ownership for binding
) -> Result<(), AppError> {
    if records.is_empty() {
        return Ok(());
    }

    // PostgreSQL has a limit of ~65535 parameters
    // We use BATCH_SIZE to control both channel batching and SQL insert size
    // Process in chunks to avoid hitting parameter limits
    while !records.is_empty() {
        let chunk_size = std::cmp::min(BATCH_SIZE, records.len());
        let chunk: Vec<T> = records.drain(..chunk_size).collect();
        bulk_insert_chunk(db, chunk).await?;
    }

    Ok(())
}

async fn batch_insert_from_channel<T: BulkInsertable>(
    mut rx: mpsc::Receiver<T>,
    db: &PgPool,
) -> Result<(), AppError> {
    let mut batch = Vec::with_capacity(BATCH_SIZE);

    while let Some(record) = rx.recv().await {
        batch.push(record);

        if batch.len() >= BATCH_SIZE {
            let current_batch = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
            insert_records(db, current_batch).await?;
        }
    }

    if !batch.is_empty() {
        insert_records(db, batch).await?;
    }

    Ok(())
}

/// Processes `DummyRecord` items from a channel and inserts them in batches.
///
/// # Errors
///
/// Returns an error if database insertion fails.
pub async fn batch_insert_dummy(
    rx: mpsc::Receiver<DummyRecord>,
    db: &PgPool,
) -> Result<(), AppError> {
    batch_insert_from_channel(rx, db).await
}

/// Processes `Gottcha2FullRecord` items from a channel and inserts them in batches.
///
/// # Errors
///
/// Returns an error if database insertion fails.
pub async fn batch_insert_gottcha2(
    rx: mpsc::Receiver<Gottcha2FullRecord>,
    db: &PgPool,
) -> Result<(), AppError> {
    batch_insert_from_channel(rx, db).await
}

/// Processes `StastRecord` items from a channel and inserts them in batches.
///
/// # Errors
///
/// Returns an error if database insertion fails.
pub async fn batch_insert_stast(
    rx: mpsc::Receiver<StastRecord>,
    db: &PgPool,
) -> Result<(), AppError> {
    batch_insert_from_channel(rx, db).await
}
