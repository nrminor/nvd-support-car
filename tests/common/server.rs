use crate::common::certificates::TestCertificates;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use nvd_support_car::{
    config::AppConfig,
    handlers::{healthz, ingest_dummy, ingest_gottcha2, ingest_stast},
    state::AppState,
};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Once;
use std::time::Duration;
use tokio::task::JoinHandle;

static CRYPTO_INIT: Once = Once::new();

fn init_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls crypto provider");
    });
}

#[allow(dead_code)]
pub struct TestServer {
    pub addr: SocketAddr,
    pub base_url: String,
    pub bearer_token: String,
    pub certs: TestCertificates,
    handle: JoinHandle<()>,
}

#[allow(dead_code)]
impl TestServer {
    pub async fn start_with_tls(db_pool: PgPool) -> Result<Self, Box<dyn std::error::Error>> {
        init_crypto_provider();

        let certs = TestCertificates::generate()?;

        let bearer_token = "test_e2e_token_secure_12345".to_string();

        let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let addr = std_listener.local_addr()?;
        std_listener.set_nonblocking(true)?;

        let config = AppConfig {
            database_url: "unused_in_tests".to_string(),
            ingest_token: bearer_token.clone(),
            server_port: addr.port(),
            cert_path: certs.cert_path.clone(),
            key_path: certs.key_path.clone(),
            rate_limit_rps: 200,
        };

        let state = AppState::new(db_pool, &config);

        let app = Router::new()
            .route("/healthz", axum::routing::get(healthz))
            .route("/ingest", axum::routing::post(ingest_dummy))
            .route("/ingest-gottcha2", axum::routing::post(ingest_gottcha2))
            .route("/ingest-stast", axum::routing::post(ingest_stast))
            .with_state(state);

        let tls_config = RustlsConfig::from_pem_file(&certs.cert_path, &certs.key_path).await?;

        let handle = tokio::spawn(async move {
            axum_server::from_tcp_rustls(std_listener, tls_config)
                .serve(app.into_make_service())
                .await
                .expect("Server failed to start");
        });

        tokio::time::sleep(Duration::from_millis(200)).await;

        Ok(TestServer {
            addr,
            base_url: format!("https://localhost:{}", addr.port()),
            bearer_token,
            certs,
            handle,
        })
    }

    pub fn create_http_client(&self) -> Result<reqwest::Client, Box<dyn std::error::Error>> {
        self.certs.create_reqwest_client()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
