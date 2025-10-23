use sqlx::PgPool;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

pub struct TestDatabase {
    pub pool: PgPool,
    pub database_url: String,
    _container: ContainerAsync<Postgres>,
}

impl TestDatabase {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let postgres_image = Postgres::default()
            .with_db_name("nvd_test")
            .with_user("postgres")
            .with_password("postgres");

        let container = postgres_image.start().await?;
        let host_port = container.get_host_port_ipv4(5432).await?;

        let database_url = format!("postgresql://postgres:postgres@localhost:{host_port}/nvd_test");

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let pool = PgPool::connect(&database_url).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(TestDatabase {
            pool,
            database_url,
            _container: container,
        })
    }

    pub async fn cleanup_tables(&self) -> Result<(), sqlx::Error> {
        sqlx::query("TRUNCATE TABLE gottcha2_results, stast_results, results CASCADE")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn count_records(&self, table: &str) -> Result<i64, sqlx::Error> {
        let query = format!("SELECT COUNT(*) FROM {table}");
        let count: (i64,) = sqlx::query_as(&query).fetch_one(&self.pool).await?;
        Ok(count.0)
    }

    pub async fn count_records_where(
        &self,
        table: &str,
        condition: &str,
        value: &str,
    ) -> Result<i64, sqlx::Error> {
        let query = format!("SELECT COUNT(*) FROM {table} WHERE {condition}");
        let count: (i64,) = sqlx::query_as(&query)
            .bind(value)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_creation() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        assert!(
            db.database_url.contains("postgresql://"),
            "Should have valid PostgreSQL URL"
        );
        assert!(
            db.database_url.contains("localhost"),
            "Should connect to localhost"
        );

        let result: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&db.pool)
            .await
            .expect("Failed to query database");

        assert_eq!(result.0, 1, "Basic query should work");
    }

    #[tokio::test]
    async fn test_migrations_applied() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT table_name FROM information_schema.tables 
             WHERE table_schema = 'public' 
             ORDER BY table_name",
        )
        .fetch_all(&db.pool)
        .await
        .expect("Failed to query tables");

        let table_names: Vec<String> = tables.into_iter().map(|t| t.0).collect();

        assert!(
            table_names.contains(&"results".to_string()),
            "Should have results table"
        );
        assert!(
            table_names.contains(&"gottcha2_results".to_string()),
            "Should have gottcha2_results table"
        );
        assert!(
            table_names.contains(&"stast_results".to_string()),
            "Should have stast_results table"
        );
    }

    #[tokio::test]
    async fn test_count_records() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        let count = db
            .count_records("gottcha2_results")
            .await
            .expect("Failed to count records");

        assert_eq!(count, 0, "Should start with zero records");
    }

    #[tokio::test]
    async fn test_cleanup_tables() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        sqlx::query(
            "INSERT INTO gottcha2_results 
             (sample_id, level, name, taxid, read_count, total_bp_mapped, 
              ani_ci95, covered_sig_len, best_sig_cov, depth, rel_abundance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind("test_sample")
        .bind("species")
        .bind("Test organism")
        .bind("12345")
        .bind(100_i64)
        .bind(5000_i64)
        .bind(0.95)
        .bind(1000_i64)
        .bind(0.85)
        .bind(10.0)
        .bind(0.25)
        .execute(&db.pool)
        .await
        .expect("Failed to insert test record");

        let count_before = db
            .count_records("gottcha2_results")
            .await
            .expect("Failed to count");
        assert_eq!(count_before, 1, "Should have 1 record after insert");

        db.cleanup_tables().await.expect("Failed to cleanup tables");

        let count_after = db
            .count_records("gottcha2_results")
            .await
            .expect("Failed to count");
        assert_eq!(count_after, 0, "Should have 0 records after cleanup");
    }

    #[tokio::test]
    async fn test_count_records_where() {
        let db = TestDatabase::new()
            .await
            .expect("Failed to create test database");

        sqlx::query(
            "INSERT INTO gottcha2_results 
             (sample_id, level, name, taxid, read_count, total_bp_mapped, 
              ani_ci95, covered_sig_len, best_sig_cov, depth, rel_abundance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind("specific_sample_001")
        .bind("species")
        .bind("Test organism")
        .bind("12345")
        .bind(100_i64)
        .bind(5000_i64)
        .bind(0.95)
        .bind(1000_i64)
        .bind(0.85)
        .bind(10.0)
        .bind(0.25)
        .execute(&db.pool)
        .await
        .expect("Failed to insert test record");

        let count = db
            .count_records_where("gottcha2_results", "sample_id = $1", "specific_sample_001")
            .await
            .expect("Failed to count with condition");

        assert_eq!(count, 1, "Should find 1 record with specific sample_id");
    }
}
