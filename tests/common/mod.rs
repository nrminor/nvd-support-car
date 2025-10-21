use async_compression::tokio::bufread::GzipDecoder;
use axum::{
    Router,
    body::Body,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use futures_util::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

pub const TEST_TOKEN: &str = "test_secret_token_12345";

// Mock handlers for testing without database or state
async fn mock_ingest_handler(headers: axum::http::HeaderMap, body: Body) -> impl IntoResponse {
    // Check authorization first
    if let Some(auth) = headers.get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str != format!("Bearer {TEST_TOKEN}") {
                return (StatusCode::UNAUTHORIZED, "Unauthorized");
            }
        } else {
            return (StatusCode::UNAUTHORIZED, "Unauthorized");
        }
    } else {
        return (StatusCode::UNAUTHORIZED, "Unauthorized");
    }

    // Try to decompress and parse the body to validate it
    let body_stream = body
        .into_data_stream()
        .map(|res| res.map_err(std::io::Error::other));

    let stream_reader = StreamReader::new(body_stream);
    let buf_reader = BufReader::new(stream_reader);
    let decoder = GzipDecoder::new(buf_reader);
    let mut lines = BufReader::new(decoder).lines();

    // Try to read at least one line to validate the data
    match lines.next_line().await {
        Ok(Some(line)) => {
            // Try to parse as JSON to validate
            if serde_json::from_str::<serde_json::Value>(&line).is_ok() {
                (StatusCode::OK, "ingested")
            } else {
                (StatusCode::BAD_REQUEST, "Invalid JSON")
            }
        }
        Ok(None) => (StatusCode::OK, "ingested"), // Empty but valid gzip
        Err(_) => (StatusCode::BAD_REQUEST, "Invalid gzip data"),
    }
}

async fn mock_healthz() -> impl IntoResponse {
    "ok"
}

pub fn create_test_app() -> Router {
    // Build the router with mock handlers - no state needed for these simple mocks
    Router::new()
        .route("/healthz", get(mock_healthz))
        .route("/ingest", post(mock_ingest_handler))
        .route("/ingest-gottcha2", post(mock_ingest_handler))
        .route("/ingest-stast", post(mock_ingest_handler))
}
