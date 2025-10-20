use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

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

    let records = match IngestService::parse_gzipped_ndjson(body).await {
        Ok(records) => records,
        Err(e) => return e.into_response(),
    };

    if let Err(e) = DbOperations::insert_records(&state.db, &records).await {
        return e.into_response();
    }

    (StatusCode::OK, "ingested").into_response()
}
