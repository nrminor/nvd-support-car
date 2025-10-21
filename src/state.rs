use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
}

impl AppState {
    #[must_use]
    pub fn new(db: sqlx::PgPool, config: &AppConfig) -> Self {
        AppState {
            db,
            config: config.clone(),
        }
    }
}
