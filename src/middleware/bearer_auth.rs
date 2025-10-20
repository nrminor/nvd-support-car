use axum::http::HeaderMap;

use crate::error::AppError;
use crate::state::AppState;

pub fn validate_bearer_token(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), AppError> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let Some(token) = auth.strip_prefix("Bearer ") else {
        return Err(AppError::Unauthorized);
    };

    if token != state.config.ingest_token {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}
