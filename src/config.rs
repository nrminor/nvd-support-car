use std::path::PathBuf;

use color_eyre::eyre::Result;
use rustls::ServerConfig;
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub ingest_token: String,
    pub server_port: u16,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    #[allow(dead_code)]
    pub rate_limit_rps: usize,
}

impl AppConfig {
    /// Creates a new `AppConfig` by reading from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if required environment variables are missing or invalid.
    pub fn new_from_env() -> Result<Self, envy::Error> {
        envy::from_env()
    }

    /// Loads TLS configuration from certificate and key files.
    ///
    /// # Errors
    ///
    /// Returns an error if certificate or key files cannot be read or parsed.
    #[allow(dead_code)]
    pub fn load_tls_config(&self) -> Result<ServerConfig> {
        let cert_file = std::fs::File::open(&self.cert_path)?;
        let mut cert_reader = std::io::BufReader::new(cert_file);
        let certs = certs(&mut cert_reader).collect::<Result<Vec<_>, _>>()?;

        let key_file = std::fs::File::open(&self.key_path)?;
        let mut key_reader = std::io::BufReader::new(key_file);
        let keys = pkcs8_private_keys(&mut key_reader).collect::<Result<Vec<_>, _>>()?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, keys[0].clone_key().into())?;

        Ok(config)
    }
}
