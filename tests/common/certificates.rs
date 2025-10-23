use rcgen::{CertificateParams, DistinguishedName, KeyPair, SanType};
use std::path::PathBuf;
use tempfile::TempDir;

pub struct TestCertificates {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub ca_cert_pem: Vec<u8>,
    _temp_dir: TempDir,
}

impl TestCertificates {
    pub fn generate() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        let mut params = CertificateParams::default();

        params.subject_alt_names = vec![
            SanType::DnsName("localhost".to_string().try_into()?),
            SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        ];

        let mut dn = DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, "NVD Support Car Test CA");
        dn.push(rcgen::DnType::OrganizationName, "NVD Test Suite");
        params.distinguished_name = dn;

        let key_pair = KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        let cert_path = temp_dir.path().join("cert.pem");
        let key_path = temp_dir.path().join("key.pem");

        std::fs::write(&cert_path, &cert_pem)?;
        std::fs::write(&key_path, &key_pem)?;

        Ok(TestCertificates {
            cert_path,
            key_path,
            ca_cert_pem: cert_pem.into_bytes(),
            _temp_dir: temp_dir,
        })
    }

    pub fn create_reqwest_client(&self) -> Result<reqwest::Client, Box<dyn std::error::Error>> {
        let cert = reqwest::Certificate::from_pem(&self.ca_cert_pem)?;

        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .add_root_certificate(cert)
            .timeout(std::time::Duration::from_secs(10))
            .gzip(true)
            .build()?;

        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_generation() {
        let certs = TestCertificates::generate().expect("Failed to generate certificates");

        assert!(certs.cert_path.exists(), "Certificate file should exist");
        assert!(certs.key_path.exists(), "Key file should exist");
        assert!(
            !certs.ca_cert_pem.is_empty(),
            "Certificate PEM should not be empty"
        );

        let cert_content =
            std::fs::read_to_string(&certs.cert_path).expect("Failed to read certificate");
        assert!(
            cert_content.contains("BEGIN CERTIFICATE"),
            "Should contain certificate header"
        );

        let key_content = std::fs::read_to_string(&certs.key_path).expect("Failed to read key");
        assert!(
            key_content.contains("BEGIN PRIVATE KEY"),
            "Should contain key header"
        );
    }

    #[test]
    fn test_reqwest_client_creation() {
        let certs = TestCertificates::generate().expect("Failed to generate certificates");
        let _client = certs
            .create_reqwest_client()
            .expect("Failed to create HTTP client");
    }

    #[test]
    fn test_temp_dir_cleanup() {
        let cert_path: PathBuf;
        let key_path: PathBuf;

        {
            let certs = TestCertificates::generate().expect("Failed to generate certificates");
            cert_path = certs.cert_path.clone();
            key_path = certs.key_path.clone();

            assert!(
                cert_path.exists(),
                "Certificate should exist while in scope"
            );
            assert!(key_path.exists(), "Key should exist while in scope");
        }

        assert!(
            !cert_path.exists(),
            "Certificate should be cleaned up after drop"
        );
        assert!(!key_path.exists(), "Key should be cleaned up after drop");
    }
}
