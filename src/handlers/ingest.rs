use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use tokio::sync::mpsc;

use crate::db::DbOperations;
use crate::middleware::validate_bearer_token;
use crate::services::IngestService;
use crate::state::AppState;

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    if let Err(e) = validate_bearer_token(&state, &headers) {
        return e.into_response();
    }

    let (tx, rx) = mpsc::channel(1000);

    let parser = IngestService::parse_gzipped_ndjson(body, tx);
    let inserter = DbOperations::batch_insert_from_channel(rx, &state.db);

    if let Err(e) = tokio::try_join!(parser, inserter) {
        return e.into_response();
    }

    (StatusCode::OK, "ingested").into_response()
}
