use async_compression::tokio::bufread::GzipDecoder;
use axum::body::Body;
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::io::StreamReader;

use crate::error::AppError;

/// Parses a gzipped JSONL body and sends each deserialized record to a channel.
///
/// # Errors
///
/// Returns an error if decompression fails, JSON parsing fails, or the channel is closed.
pub async fn parse_gzipped_jsonl<T>(body: Body, tx: mpsc::Sender<T>) -> Result<(), AppError>
where
    T: serde::de::DeserializeOwned,
{
    let body_stream = body
        .into_data_stream()
        .map(|res| res.map_err(std::io::Error::other));

    let stream_reader = StreamReader::new(body_stream);
    let buf_reader = BufReader::new(stream_reader);
    let decoder = GzipDecoder::new(buf_reader);

    let mut jsonl_lines = BufReader::new(decoder).lines();

    while let Ok(Some(line)) = jsonl_lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let rec = serde_json::from_str::<T>(&line)
            .map_err(|e| AppError::BadRequest(format!("invalid json line: {e}")))?;

        tx.send(rec)
            .await
            .map_err(|_| AppError::InternalServerError("channel closed".to_string()))?;
    }

    Ok(())
}
