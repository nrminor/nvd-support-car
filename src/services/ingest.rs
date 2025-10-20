use async_compression::tokio::bufread::GzipDecoder;
use axum::body::Body;
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

use crate::error::AppError;
use crate::models::DummyRecord;

pub struct IngestService;

impl IngestService {
    pub async fn parse_gzipped_ndjson(body: Body) -> Result<Vec<DummyRecord>, AppError> {
        let body_stream = body
            .into_data_stream()
            .map(|res| res.map_err(std::io::Error::other));

        let stream_reader = StreamReader::new(body_stream);
        let buf_reader = BufReader::new(stream_reader);
        let decoder = GzipDecoder::new(buf_reader);

        let mut jsonl_lines = BufReader::new(decoder).lines();
        let mut records = Vec::new();

        while let Ok(Some(line)) = jsonl_lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            let rec = serde_json::from_str::<DummyRecord>(&line)
                .map_err(|_| AppError::BadRequest("invalid json line".to_string()))?;

            records.push(rec);
        }

        Ok(records)
    }
}
