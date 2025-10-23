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

pub struct TestServer {
    pub addr: SocketAddr,
    pub base_url: String,
    pub bearer_token: String,
    pub certs: TestCertificates,
    handle: JoinHandle<()>,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::database::TestDatabase;

    #[tokio::test]
    async fn test_server_starts_successfully() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let server = TestServer::start_with_tls(db.pool.clone())
            .await
            .expect("Failed to start test server");

        assert!(server.addr.port() > 0, "Server should have a valid port");
        assert!(
            server.base_url.starts_with("https://localhost:"),
            "Base URL should be HTTPS"
        );
        assert!(
            !server.bearer_token.is_empty(),
            "Bearer token should be set"
        );
    }

    #[tokio::test]
    async fn test_server_health_check_over_tls() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let server = TestServer::start_with_tls(db.pool.clone())
            .await
            .expect("Failed to start test server");

        let client = server
            .create_http_client()
            .expect("Failed to create HTTP client");

        let base_url = &server.base_url;
        let response = client
            .get(format!("{base_url}/healthz"))
            .send()
            .await
            .expect("Health check request failed");

        assert_eq!(response.status(), 200, "Health check should return 200");

        let body = response.text().await.expect("Failed to read response body");
        assert_eq!(body, "ok", "Health check should return 'ok'");
    }

    #[tokio::test]
    async fn test_tls_handshake_succeeds() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let server = TestServer::start_with_tls(db.pool.clone())
            .await
            .expect("Failed to start test server");

        let client = server
            .create_http_client()
            .expect("Failed to create HTTP client");

        let base_url = &server.base_url;
        let result = client.get(format!("{base_url}/healthz")).send().await;

        assert!(
            result.is_ok(),
            "TLS handshake should succeed with custom CA"
        );
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let server = TestServer::start_with_tls(db.pool.clone())
            .await
            .expect("Failed to start test server");

        let client = server
            .create_http_client()
            .expect("Failed to create HTTP client");

        let base_url = &server.base_url;
        let response = client
            .get(format!("{base_url}/healthz"))
            .send()
            .await
            .expect("Request should succeed while server is running");

        assert_eq!(
            response.status(),
            200,
            "Server should be accessible while running"
        );

        assert!(
            !server.handle.is_finished(),
            "Server task should still be running"
        );
    }

    #[tokio::test]
    async fn test_multiple_servers_parallel() {
        let db1 = TestDatabase::new()
            .await
            .expect("Failed to create first database");
        let db2 = TestDatabase::new()
            .await
            .expect("Failed to create second database");

        let server1 = TestServer::start_with_tls(db1.pool.clone())
            .await
            .expect("Failed to start first server");

        let server2 = TestServer::start_with_tls(db2.pool.clone())
            .await
            .expect("Failed to start second server");

        assert_ne!(
            server1.addr.port(),
            server2.addr.port(),
            "Servers should have different ports"
        );

        let client1 = server1
            .create_http_client()
            .expect("Failed to create client 1");
        let client2 = server2
            .create_http_client()
            .expect("Failed to create client 2");

        let base_url1 = &server1.base_url;
        let base_url2 = &server2.base_url;

        let response1 = client1
            .get(format!("{base_url1}/healthz"))
            .send()
            .await
            .expect("Server 1 health check failed");

        let response2 = client2
            .get(format!("{base_url2}/healthz"))
            .send()
            .await
            .expect("Server 2 health check failed");

        assert_eq!(response1.status(), 200, "Server 1 should respond");
        assert_eq!(response2.status(), 200, "Server 2 should respond");
    }

    #[tokio::test]
    async fn test_tls_without_trusted_ca_fails() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let server = TestServer::start_with_tls(db.pool.clone())
            .await
            .expect("Failed to start test server");

        let untrusted_client = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build client");

        let base_url = &server.base_url;
        let result = untrusted_client
            .get(format!("{base_url}/healthz"))
            .send()
            .await;

        assert!(
            result.is_err(),
            "Connection should fail without trusting test CA"
        );

        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("certificate")
                    || error_msg.contains("UnknownIssuer")
                    || error_msg.contains("InvalidCertificate")
                    || error_msg.contains("error sending request"),
                "Error should be certificate or TLS-related, got: {error_msg}"
            );
        }
    }
}
