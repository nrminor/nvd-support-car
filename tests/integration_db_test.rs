mod common;

use common::database::TestDatabase;
use nvd_support_car::{
    db::operations::{batch_insert_gottcha2, batch_insert_stast},
    models::record::{Gottcha2FullRecord, StastRecord},
};
use tokio::sync::mpsc;

#[tokio::test]
async fn test_batch_insert_gottcha2_with_real_db() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let (tx, rx) = mpsc::channel(100);

    let pool = db.pool.clone();
    let insert_handle = tokio::spawn(async move { batch_insert_gottcha2(rx, &pool).await });

    for i in 0..50 {
        let record = Gottcha2FullRecord {
            sample_id: format!("batch_test_{i:03}"),
            level: "species".to_string(),
            name: format!("Species_{i}"),
            taxid: format!("{}", 20000 + i),
            read_count: 100,
            total_bp_mapped: 5000,
            ani_ci95: 0.95,
            covered_sig_len: 1000,
            best_sig_cov: 0.85,
            depth: 10.0,
            rel_abundance: 0.1,
        };
        tx.send(record).await.expect("Failed to send record");
    }

    drop(tx);
    let result = insert_handle.await.expect("Insert task panicked");
    assert!(result.is_ok(), "Batch insert should succeed");

    let count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count records");
    assert_eq!(count, 50, "Expected 50 records in database");

    let first_sample: String =
        sqlx::query_scalar("SELECT sample_id FROM gottcha2_results ORDER BY id LIMIT 1")
            .fetch_one(&db.pool)
            .await
            .expect("Failed to fetch first sample");
    assert_eq!(first_sample, "batch_test_000");
}

#[tokio::test]
async fn test_batch_insert_stast_with_real_db() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let (tx, rx) = mpsc::channel(100);

    let pool = db.pool.clone();
    let insert_handle = tokio::spawn(async move { batch_insert_stast(rx, &pool).await });

    for i in 0..30 {
        let record = StastRecord {
            task: "megablast".to_string(),
            sample_id: format!("stast_batch_{i:03}"),
            qseqid: format!("NODE_{i}_length_1000"),
            qlen: 1000,
            sseqid: format!("gi|{i}|ref|NC_000001.1|"),
            stitle: format!("Test virus {i}"),
            length: 950,
            pident: 99.5,
            evalue: 0.0,
            bitscore: 1800.0,
            sscinames: "Test virus".to_string(),
            staxids: "12345".to_string(),
            rank: "species:Test virus".to_string(),
        };
        tx.send(record).await.expect("Failed to send record");
    }

    drop(tx);
    let result = insert_handle.await.expect("Insert task panicked");
    assert!(result.is_ok(), "Batch insert should succeed");

    let count = db
        .count_records("stast_results")
        .await
        .expect("Failed to count records");
    assert_eq!(count, 30, "Expected 30 records in database");
}

#[tokio::test]
async fn test_concurrent_database_operations() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let mut handles = vec![];

    for i in 0..5 {
        let pool = db.pool.clone();
        handles.push(tokio::spawn(async move {
            let (tx, rx) = mpsc::channel(10);

            let insert_pool = pool.clone();
            let inserter =
                tokio::spawn(async move { batch_insert_gottcha2(rx, &insert_pool).await });

            for j in 0..10 {
                let record = Gottcha2FullRecord {
                    sample_id: format!("concurrent_g2_{i:02}_{j:02}"),
                    level: "species".to_string(),
                    name: "Test".to_string(),
                    taxid: "1".to_string(),
                    read_count: 100,
                    total_bp_mapped: 5000,
                    ani_ci95: 0.95,
                    covered_sig_len: 1000,
                    best_sig_cov: 0.85,
                    depth: 10.0,
                    rel_abundance: 0.1,
                };
                tx.send(record).await.expect("Failed to send record");
            }
            drop(tx);

            inserter.await.expect("Inserter task panicked")
        }));
    }

    for i in 0..5 {
        let pool = db.pool.clone();
        handles.push(tokio::spawn(async move {
            let (tx, rx) = mpsc::channel(10);

            let insert_pool = pool.clone();
            let inserter = tokio::spawn(async move { batch_insert_stast(rx, &insert_pool).await });

            for j in 0..10 {
                let record = StastRecord {
                    sample_id: format!("concurrent_st_{i:02}_{j:02}"),
                    task: "blast".to_string(),
                    qseqid: "query".to_string(),
                    qlen: 100,
                    sseqid: "subject".to_string(),
                    stitle: "title".to_string(),
                    length: 90,
                    pident: 95.0,
                    evalue: 0.001,
                    bitscore: 100.0,
                    sscinames: "species".to_string(),
                    staxids: "1".to_string(),
                    rank: "species".to_string(),
                };
                tx.send(record).await.expect("Failed to send record");
            }
            drop(tx);

            inserter.await.expect("Inserter task panicked")
        }));
    }

    for handle in handles {
        let result = handle.await.expect("Task panicked");
        assert!(result.is_ok(), "Concurrent insert should succeed");
    }

    let gottcha2_count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count gottcha2");
    let stast_count = db
        .count_records("stast_results")
        .await
        .expect("Failed to count stast");

    assert_eq!(gottcha2_count, 50, "Expected 50 GOTTCHA2 records");
    assert_eq!(stast_count, 50, "Expected 50 STAST records");
}

#[tokio::test]
async fn test_batch_efficiency_large_dataset() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let (tx, rx) = mpsc::channel(1000);

    let pool = db.pool.clone();
    let start = std::time::Instant::now();
    let insert_handle = tokio::spawn(async move { batch_insert_gottcha2(rx, &pool).await });

    for i in 0..1000 {
        let record = Gottcha2FullRecord {
            sample_id: format!("large_batch_{i:04}"),
            level: "species".to_string(),
            name: format!("Species_{i}"),
            taxid: format!("{}", 30000 + i),
            read_count: 100 + i64::from(i),
            total_bp_mapped: 5000 + i64::from(i) * 10,
            ani_ci95: 0.95,
            covered_sig_len: 1000,
            best_sig_cov: 0.85,
            depth: 10.0 + (f64::from(i) * 0.1),
            rel_abundance: 0.1,
        };
        tx.send(record).await.expect("Failed to send record");
    }

    drop(tx);
    let result = insert_handle.await.expect("Insert task panicked");
    let duration = start.elapsed();

    assert!(result.is_ok(), "Large batch insert should succeed");

    let count = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count records");
    assert_eq!(count, 1000, "Expected 1000 records in database");

    assert!(
        duration.as_secs() < 10,
        "Batch insert should complete in under 10 seconds, took {duration:?}"
    );
}

#[tokio::test]
async fn test_data_integrity_after_insert() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let (tx, rx) = mpsc::channel(10);

    let pool = db.pool.clone();
    let insert_handle = tokio::spawn(async move { batch_insert_gottcha2(rx, &pool).await });

    let test_record = Gottcha2FullRecord {
        sample_id: "integrity_test_001".to_string(),
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
    };

    tx.send(test_record.clone())
        .await
        .expect("Failed to send record");
    drop(tx);

    insert_handle
        .await
        .expect("Insert task panicked")
        .expect("Insert should succeed");

    let retrieved: (
        String,
        String,
        String,
        String,
        i64,
        i64,
        f64,
        i64,
        f64,
        f64,
        f64,
    ) = sqlx::query_as(
        "SELECT sample_id, level, name, taxid, read_count, total_bp_mapped, 
                    ani_ci95, covered_sig_len, best_sig_cov, depth, rel_abundance 
             FROM gottcha2_results 
             WHERE sample_id = $1",
    )
    .bind(&test_record.sample_id)
    .fetch_one(&db.pool)
    .await
    .expect("Failed to retrieve record");

    assert_eq!(retrieved.0, test_record.sample_id);
    assert_eq!(retrieved.1, test_record.level);
    assert_eq!(retrieved.2, test_record.name);
    assert_eq!(retrieved.3, test_record.taxid);
    assert_eq!(retrieved.4, test_record.read_count);
    assert_eq!(retrieved.5, test_record.total_bp_mapped);
    assert!((retrieved.6 - test_record.ani_ci95).abs() < 0.001);
    assert_eq!(retrieved.7, test_record.covered_sig_len);
    assert!((retrieved.8 - test_record.best_sig_cov).abs() < 0.001);
    assert!((retrieved.9 - test_record.depth).abs() < 0.001);
    assert!((retrieved.10 - test_record.rel_abundance).abs() < 0.001);
}

#[tokio::test]
async fn test_table_cleanup_between_tests() {
    let db = TestDatabase::new()
        .await
        .expect("Failed to create test database");

    let (tx, rx) = mpsc::channel(10);
    let pool = db.pool.clone();
    let insert_handle = tokio::spawn(async move { batch_insert_gottcha2(rx, &pool).await });

    for i in 0..5 {
        let record = Gottcha2FullRecord {
            sample_id: format!("cleanup_test_{i}"),
            level: "species".to_string(),
            name: "Test".to_string(),
            taxid: "1".to_string(),
            read_count: 100,
            total_bp_mapped: 5000,
            ani_ci95: 0.95,
            covered_sig_len: 1000,
            best_sig_cov: 0.85,
            depth: 10.0,
            rel_abundance: 0.1,
        };
        tx.send(record).await.expect("Failed to send record");
    }
    drop(tx);

    insert_handle
        .await
        .expect("Insert task panicked")
        .expect("Insert should succeed");

    let count_before = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count_before, 5, "Should have 5 records before cleanup");

    db.cleanup_tables().await.expect("Cleanup should succeed");

    let count_after = db
        .count_records("gottcha2_results")
        .await
        .expect("Failed to count");
    assert_eq!(count_after, 0, "Should have 0 records after cleanup");
}
