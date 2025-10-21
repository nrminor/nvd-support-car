use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use tokio::sync::mpsc;

use crate::{
    db::operations::batch_insert_dummy, middleware::validate_bearer_token,
    services::parsing::parse_gzipped_jsonl, state::AppState,
};

pub async fn ingest_dummy(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    if let Err(e) = validate_bearer_token(&state, &headers) {
        return e.into_response();
    }

    let (tx, rx) = mpsc::channel(1000);

    let parser = parse_gzipped_jsonl(body, tx);
    let inserter = batch_insert_dummy(rx, &state.db);

    if let Err(e) = tokio::try_join!(parser, inserter) {
        return e.into_response();
    }

    (StatusCode::OK, "ingested").into_response()
}
