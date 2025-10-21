use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Router,
    routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
use color_eyre::eyre::{Result, eyre};
use tower::ServiceBuilder;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::timeout::TimeoutLayer;

mod config;
mod db;
mod error;
mod handlers;
mod middleware;
mod models;
mod preflight;
mod services;
mod state;

use config::AppConfig;
use handlers::{healthz, ingest_dummy, ingest_gottcha2, ingest_stast};
use state::AppState;

/// Main entry point for the NVD support car server.
///
/// # Errors
///
/// Returns an error if:
/// - Environment variables cannot be loaded
/// - Database connection fails
/// - TLS certificates cannot be loaded
/// - Server fails to start
///
/// # Panics
///
/// Panics if the governor rate limiter configuration fails to build.
#[tokio::main]
pub async fn main() -> Result<()> {
    // let the user now we've started
    tracing::info!(
        "You've launched to the NVD support car, designed to support the NVD metagenomic pipeline as it races to identify human virus-family pathogens in big sequence datasets. Proceeding to preflight checks..."
    );

    // run preflight checks
    preflight::setup_tracing();
    preflight::init_error_formatter()?;
    preflight::checks();
    tracing::info!("All preflight checks passed. Proceeding to server setup");

    // set up app configs
    tracing::info!("Setting up application configuration from environment variables.");
    let config = AppConfig::new_from_env()?;

    // connect the database
    tracing::info!("Configuration Set. Proceeding to launching a database connecton pool...");
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(30)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&config.database_url)
        .await?;

    // run migrations if need
    tracing::info!(
        "Connection pool successfully launched. Running database migrations as necessary..."
    );
    sqlx::migrate!().run(&db).await?;

    // set up rate-limiting governor
    tracing::info!("Configuring the connection rate-limiter...");
    let governer = GovernorConfigBuilder::default()
        .per_second(200)
        .burst_size(400)
        .finish()
        .ok_or_else(|| eyre!("Failed to build governor config"))?;

    // launch router with the governor as a service layer (dependency injection is a cool pttern)
    tracing::info!("Rate-limiting configured. Configuring router...");
    let state = AppState::new(db, &config);
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/ingest", post(ingest_dummy))
        .route("/ingest-gottcha2", post(ingest_gottcha2))
        .route("/ingest-stast", post(ingest_stast))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(GovernorLayer::new(Arc::new(governer)))
                .layer(TimeoutLayer::new(Duration::from_secs(5))),
        );

    // set up certificates
    tracing::info!("Reading certificates for forming secure TLS connections while NVD runs.");
    let tls = RustlsConfig::from_pem_file(config.cert_path, config.key_path).await?;

    // but the address and report and run the server
    tracing::info!(
        "All setup is complete. The support car is ready. Now attaching to the 0.0.0.0 address at port {}, where the support care will await requests.",
        config.server_port
    );
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    axum_server::bind_rustls(addr, tls)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
