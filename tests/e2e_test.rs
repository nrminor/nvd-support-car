mod common;

use common::{database::TestDatabase, server::TestServer};
use flate2::{Compression, write::GzEncoder};
use nvd_support_car::models::record::{Gottcha2FullRecord, StastRecord};
use reqwest::StatusCode;
use std::io::Write;

#[tokio::test]
async fn test_e2e_gottcha2_ingestion_with_tls() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let records = vec![
        Gottcha2FullRecord {
            sample_id: "e2e_test_sample_001".to_string(),
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
            sample_id: "e2e_test_sample_001".to_string(),
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

    let jsonl = records
        .iter()
        .map(|r| serde_json::to_string(r).expect("Failed to serialize"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(jsonl.as_bytes())
        .expect("Failed to write to encoder");
    let compressed = encoder.finish().expect("Failed to finish compression");

    let base_url = &server.base_url;
    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .header("Content-Type", "application/gzip")
        .body(compressed)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.text().await.expect("Failed to read body"),
        "ingested"
    );

    let count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count, 2, "Expected 2 records in database");

    let sample_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gottcha2_results WHERE sample_id = $1")
            .bind("e2e_test_sample_001")
            .fetch_one(&db.pool)
            .await
            .expect("Failed to query");
    assert_eq!(sample_count, 2);
}

#[tokio::test]
async fn test_e2e_stast_ingestion_with_tls() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let records = vec![StastRecord {
        task: "megablast".to_string(),
        sample_id: "stast_e2e_test_001".to_string(),
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

    let jsonl = records
        .iter()
        .map(|r| serde_json::to_string(r).expect("Failed to serialize"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(jsonl.as_bytes())
        .expect("Failed to write to encoder");
    let compressed = encoder.finish().expect("Failed to finish compression");

    let base_url = &server.base_url;
    let response = client
        .post(format!("{base_url}/ingest-stast"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .header("Content-Type", "application/gzip")
        .body(compressed)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let count = db
        .count_records("stast_results")
        .await
        .expect("Failed to count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_e2e_authentication_over_tls() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let base_url = &server.base_url;

    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", "Bearer invalid_token_12345")
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .body(vec![1, 2, 3, 4])
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Invalid gzip currently succeeds with 0 records (known behavior)"
    );

    let count_after = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count_after, 0, "Invalid gzip should insert 0 records");
}

#[tokio::test]
async fn test_e2e_concurrent_ingestion() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");

    let mut handles = vec![];

    for i in 0..10 {
        let client = server
            .create_http_client()
            .expect("Failed to create client");
        let url = format!("{}/ingest-gottcha2", server.base_url);
        let token = server.bearer_token.clone();

        let handle = tokio::spawn(async move {
            let record = Gottcha2FullRecord {
                sample_id: format!("concurrent_sample_{i:03}"),
                level: "species".to_string(),
                name: format!("Species_{i}"),
                taxid: format!("{}", 10000 + i),
                read_count: 100 * i64::from(i),
                total_bp_mapped: 5000 * i64::from(i),
                ani_ci95: 0.95,
                covered_sig_len: 1000,
                best_sig_cov: 0.85,
                depth: 10.0,
                rel_abundance: 0.1,
            };

            let jsonl = serde_json::to_string(&record).expect("Failed to serialize");
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(jsonl.as_bytes())
                .expect("Failed to write");
            let compressed = encoder.finish().expect("Failed to compress");

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/gzip")
                .body(compressed)
                .send()
                .await
                .expect("Request failed");

            response.status()
        });

        handles.push(handle);
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let status = handle.await.expect("Task panicked");
        assert_eq!(status, StatusCode::OK, "Request {i} failed");
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT sample_id) FROM gottcha2_results 
         WHERE sample_id LIKE 'concurrent_sample_%'",
    )
    .fetch_one(&db.pool)
    .await
    .expect("Failed to query");

    assert_eq!(count, 10, "Expected 10 distinct samples in database");
}

#[tokio::test]
async fn test_e2e_tls_certificate_validation() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");

    let untrusting_client = reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("Failed to build client");

    let base_url = &server.base_url;
    let result = untrusting_client
        .get(format!("{base_url}/healthz"))
        .send()
        .await;

    assert!(result.is_err(), "Expected certificate validation to fail");
    let err = result.expect_err("Should have certificate error");
    let error_msg = err.to_string();
    assert!(
        error_msg.contains("certificate")
            || error_msg.contains("InvalidCertificate")
            || error_msg.contains("error sending request"),
        "Expected certificate error, got: {error_msg}"
    );
}

#[tokio::test]
async fn test_e2e_malformed_data_handling() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let base_url = &server.base_url;

    let invalid_gzip = vec![1, 2, 3, 4, 5];
    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .body(invalid_gzip)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Invalid gzip currently succeeds with 0 records"
    );

    let invalid_json = "{ invalid json }\nnot even json\n";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(invalid_json.as_bytes())
        .expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to compress");

    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .body(compressed)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_e2e_empty_payload_handling() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let empty_data = String::new();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(empty_data.as_bytes())
        .expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to compress");

    let base_url = &server.base_url;
    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .header("Content-Type", "application/gzip")
        .body(compressed)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count, 0, "Empty payload should insert 0 records");
}

#[tokio::test]
async fn test_e2e_large_payload() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");
    let server = TestServer::start_with_tls(db.pool.clone())
        .await
        .expect("Failed to start server");
    let client = server
        .create_http_client()
        .expect("Failed to create client");

    let mut records = Vec::new();
    for i in 0..100 {
        records.push(Gottcha2FullRecord {
            sample_id: format!("large_payload_{i:03}"),
            level: "species".to_string(),
            name: format!("Species_{i}"),
            taxid: format!("{}", 40000 + i),
            read_count: 100,
            total_bp_mapped: 5000,
            ani_ci95: 0.95,
            covered_sig_len: 1000,
            best_sig_cov: 0.85,
            depth: 10.0,
            rel_abundance: 0.1,
        });
    }

    let jsonl = records
        .iter()
        .map(|r| serde_json::to_string(r).expect("Failed to serialize"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(jsonl.as_bytes())
        .expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to compress");

    let base_url = &server.base_url;
    let response = client
        .post(format!("{base_url}/ingest-gottcha2"))
        .header("Authorization", format!("Bearer {}", server.bearer_token))
        .header("Content-Type", "application/gzip")
        .body(compressed)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count, 100, "Expected 100 records from large payload");
}
