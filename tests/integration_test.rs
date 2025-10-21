use axum::body::Bytes;
use axum_test::TestServer;
use flate2::Compression;
use flate2::write::GzEncoder;
use nvd_support_car::models::record::{Gottcha2FullRecord, StastRecord};
use std::io::Write;

mod common;
use common::{TEST_TOKEN, create_test_app};

#[tokio::test]
async fn test_healthz_endpoint() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    let response = server.get("/healthz").await;

    response.assert_status_ok();
    response.assert_text("ok");
}

#[tokio::test]
async fn test_gottcha2_endpoint_with_valid_data() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Create test GOTTCHA2 records
    let records = vec![
        Gottcha2FullRecord {
            sample_id: "test_sample_001".to_string(),
            level: "phylum".to_string(),
            name: "Proteobacteria".to_string(),
            taxid: "1224".to_string(),
            read_count: 1000,
            total_bp_mapped: 50000,
            ani_ci95: 0.95,
            covered_sig_len: 3000,
            best_sig_cov: 0.85,
            depth: 10.5,
            rel_abundance: 0.25,
        },
        Gottcha2FullRecord {
            sample_id: "test_sample_001".to_string(),
            level: "genus".to_string(),
            name: "Escherichia".to_string(),
            taxid: "561".to_string(),
            read_count: 500,
            total_bp_mapped: 25000,
            ani_ci95: 0.98,
            covered_sig_len: 1500,
            best_sig_cov: 0.90,
            depth: 15.2,
            rel_abundance: 0.12,
        },
    ];

    // Convert to JSONL
    let mut jsonl = String::new();
    for record in &records {
        jsonl.push_str(&serde_json::to_string(&record).expect("Failed to serialize record"));
        jsonl.push('\n');
    }

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(jsonl.as_bytes())
        .expect("Failed to write to gzip encoder");
    let compressed_data = encoder.finish().expect("Failed to finish gzip compression");

    // Send request with bearer token
    let response = server
        .post("/ingest-gottcha2")
        .add_header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .bytes(Bytes::from(compressed_data))
        .await;

    response.assert_status_ok();
    response.assert_text("ingested");
}

#[tokio::test]
async fn test_stast_endpoint_with_valid_data() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Create test STAST records
    let records = vec![StastRecord {
        task: "megablast".to_string(),
        sample_id: "test_sample_002".to_string(),
        qseqid: "NODE_1_length_1000".to_string(),
        qlen: 1000,
        sseqid: "gi|123456|ref|NC_000001.1|".to_string(),
        stitle: "Test virus genome".to_string(),
        length: 950,
        pident: 99.5,
        evalue: 0.0,
        bitscore: 1800.0,
        sscinames: "Test virus".to_string(),
        staxids: "12345".to_string(),
        rank: "species:Test virus".to_string(),
    }];

    // Convert to JSONL
    let mut jsonl = String::new();
    for record in &records {
        jsonl.push_str(&serde_json::to_string(&record).expect("Failed to serialize record"));
        jsonl.push('\n');
    }

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(jsonl.as_bytes())
        .expect("Failed to write to gzip encoder");
    let compressed_data = encoder.finish().expect("Failed to finish gzip compression");

    // Send request with bearer token
    let response = server
        .post("/ingest-stast")
        .add_header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .bytes(Bytes::from(compressed_data))
        .await;

    response.assert_status_ok();
    response.assert_text("ingested");
}

#[tokio::test]
async fn test_authentication_missing_token() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Send request without token
    let response = server
        .post("/ingest-gottcha2")
        .bytes(Bytes::from(vec![]))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_authentication_invalid_token() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Send request with wrong token
    let response = server
        .post("/ingest-gottcha2")
        .add_header("Authorization", "Bearer wrong_token_12345")
        .bytes(Bytes::from(vec![]))
        .await;

    response.assert_status_unauthorized();
}

#[tokio::test]
async fn test_malformed_gzip_data() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Send invalid gzip data
    let invalid_gzip = vec![1, 2, 3, 4, 5];

    let response = server
        .post("/ingest-gottcha2")
        .add_header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .bytes(Bytes::from(invalid_gzip))
        .await;

    response.assert_status_bad_request();
}

#[tokio::test]
async fn test_invalid_json_in_gzipped_data() {
    let server = TestServer::new(create_test_app()).expect("Failed to create test server");

    // Create invalid JSON
    let invalid_json = "{ invalid json }\nnot even json\n";

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(invalid_json.as_bytes())
        .expect("Failed to write invalid data to encoder");
    let compressed_data = encoder.finish().expect("Failed to finish gzip compression");

    let response = server
        .post("/ingest-gottcha2")
        .add_header("Authorization", format!("Bearer {TEST_TOKEN}"))
        .bytes(Bytes::from(compressed_data))
        .await;

    response.assert_status_bad_request();
}
