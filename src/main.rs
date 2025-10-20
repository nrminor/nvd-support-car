use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::Router;
use axum::routing::{get, post};
use axum_server::tls_rustls::RustlsConfig;
use color_eyre::eyre::{Result, eyre};
use tower::ServiceBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::timeout::TimeoutLayer;

mod config;
mod state;
mod error;
mod models;
mod services;
mod db;
mod middleware;
mod handlers;
mod preflight;

use config::AppConfig;
use state::AppState;
use handlers::{healthz, ingest};

#[tokio::main]
pub async fn main() -> Result<()> {
    preflight::setup_tracing();
    preflight::init_error_formatter()?;
    preflight::checks();

    let config = AppConfig::new_from_env()?;

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(30)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!().run(&db).await?;

    let governer = GovernorConfigBuilder::default()
        .per_second(200)
        .burst_size(400)
        .finish()
        .ok_or_else(|| eyre!("Failed to build governor config"))?;

    let state = AppState::new(db, &config);
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/ingest", post(ingest))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(GovernorLayer::new(Arc::new(governer)))
                .layer(TimeoutLayer::new(Duration::from_secs(5))),
        );

    let tls = RustlsConfig::from_pem_file(config.cert_path, config.key_path).await?;

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    axum_server::bind_rustls(addr, tls)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
