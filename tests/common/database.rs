use sqlx::PgPool;
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

#[allow(dead_code)]
pub struct TestDatabase {
    pub pool: PgPool,
    pub database_url: String,
    _container: ContainerAsync<Postgres>,
}

#[allow(dead_code)]
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
