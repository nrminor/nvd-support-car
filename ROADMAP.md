# NVD Support Car - Feature Roadmap

## 1. OpenAPI Specification

### Overview

Add OpenAPI 3.0 specification to document and expose the API, enabling automatic
client generation and interactive documentation.

### Implementation Approach

#### Dependencies

```toml
# Cargo.toml additions
utoipa = { version = "4", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "6", features = ["axum"] }
utoipa-redoc = { version = "3", features = ["axum"] }
```

#### Code Structure

```rust
// src/openapi.rs
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health::healthz,
        handlers::dummy::ingest,
        handlers::gottcha2::ingest_gottcha2,
        handlers::stast::ingest_stast,
        handlers::export::export_gottcha2_parquet,
        handlers::export::export_stast_parquet,
    ),
    components(schemas(
        DummyRecord,
        Gottcha2Record,
        StastRecord,
        ExportRequest,
        ExportResponse,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "ingestion", description = "Data ingestion endpoints"),
        (name = "export", description = "Data export endpoints"),
    ),
    info(
        title = "NVD Support Car API",
        version = "0.1.0",
        description = "Backend service for ingesting metagenomic data from HTCondor nodes",
        contact(
            name = "NVD Support Team",
            email = "support@example.com",
        ),
        license(
            name = "MIT",
        ),
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development"),
        (url = "https://api.nvd-support.org", description = "Production"),
    ),
)]
pub struct ApiDoc;
```

#### Endpoint Documentation Example

```rust
/// Ingest GOTTCHA2 taxonomic abundance data
#[utoipa::path(
    post,
    path = "/ingest-gottcha2",
    tag = "ingestion",
    request_body(
        content = Vec<u8>,
        content_type = "application/gzip",
        description = "Gzipped JSONL containing GOTTCHA2 records"
    ),
    responses(
        (status = 200, description = "Data successfully ingested"),
        (status = 401, description = "Invalid or missing bearer token"),
        (status = 400, description = "Invalid data format"),
        (status = 500, description = "Internal server error"),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ingest_gottcha2(/* ... */) -> impl IntoResponse {
    // existing implementation
}
```

#### Serving Documentation

```rust
// In main.rs
app
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
    .merge(Redoc::with_url("/redoc", ApiDoc::openapi()))
    .route("/api-docs/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
```

### Benefits

- Auto-generated interactive documentation at `/swagger-ui` and `/redoc`
- OpenAPI spec available at `/api-docs/openapi.json`
- Enables client SDK generation for multiple languages
- Type-safe API documentation that stays in sync with code

---

## 2. Parquet Export Functionality

### Overview

Periodically export GOTTCHA2 and STAST tables as compressed Parquet files for
efficient data distribution and analysis.

### Implementation Approach

#### Dependencies

```toml
# Cargo.toml additions
arrow = "50"
parquet = { version = "50", features = ["async", "compression"] }
tokio-cron-scheduler = "0.10"
object_store = { version = "0.10", features = ["aws", "gcp", "azure"] }
```

#### Export Service Structure

```rust
// src/services/export.rs
use arrow::record_batch::RecordBatch;
use parquet::arrow::AsyncArrowWriter;
use parquet::file::properties::{WriterProperties, WriterVersion};
use parquet::basic::{Compression, Encoding};

pub struct ParquetExportService {
    db: PgPool,
    export_config: ExportConfig,
}

#[derive(Clone, Deserialize)]
pub struct ExportConfig {
    pub compression: CompressionType,
    pub batch_size: usize,
    pub export_interval_hours: u32,
    pub retention_days: u32,
    pub storage_backend: StorageBackend,
}

#[derive(Clone, Deserialize)]
pub enum CompressionType {
    Snappy,
    Gzip,
    Zstd(i32), // compression level
    Lz4,
}

#[derive(Clone, Deserialize)]
pub enum StorageBackend {
    LocalFS { path: PathBuf },
    S3 { bucket: String, prefix: String },
    Azure { container: String, prefix: String },
}
```

#### Export Implementation

```rust
impl ParquetExportService {
    pub async fn export_gottcha2(&self) -> Result<ExportMetadata, AppError> {
        // 1. Stream data from PostgreSQL
        let mut stream = sqlx::query_as::<_, Gottcha2Record>(
            "SELECT * FROM gottcha2_full ORDER BY created_at"
        )
        .fetch(&self.db);
        
        // 2. Convert to Arrow format
        let schema = Arc::new(self.gottcha2_arrow_schema());
        let mut batches = Vec::new();
        let mut current_batch = Vec::new();
        
        while let Some(record) = stream.try_next().await? {
            current_batch.push(record);
            if current_batch.len() >= self.export_config.batch_size {
                batches.push(self.records_to_arrow_batch(&current_batch, &schema)?);
                current_batch.clear();
            }
        }
        
        // 3. Write to Parquet with compression
        let file_name = format!("gottcha2_{}.parquet", Utc::now().format("%Y%m%d_%H%M%S"));
        let mut buffer = Vec::new();
        
        let props = WriterProperties::builder()
            .set_compression(self.map_compression())
            .set_encoding(Encoding::DELTA_BINARY_PACKED)
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_data_pagesize_limit(1024 * 1024) // 1MB pages
            .build();
            
        let mut writer = AsyncArrowWriter::try_new(
            &mut buffer,
            schema.clone(),
            Some(props),
        )?;
        
        for batch in batches {
            writer.write(&batch).await?;
        }
        writer.close().await?;
        
        // 4. Upload to storage backend
        let url = self.upload_to_storage(&file_name, buffer).await?;
        
        Ok(ExportMetadata {
            file_name,
            url,
            size_bytes: buffer.len(),
            record_count,
            compression: self.export_config.compression.clone(),
            created_at: Utc::now(),
        })
    }
}
```

#### Scheduled Export Job

```rust
// src/jobs/export_scheduler.rs
use tokio_cron_scheduler::{Job, JobScheduler};

pub async fn setup_export_scheduler(
    export_service: Arc<ParquetExportService>,
) -> Result<(), AppError> {
    let scheduler = JobScheduler::new().await?;
    
    // Schedule GOTTCHA2 export every N hours
    let gottcha2_job = Job::new_async(
        format!("0 0 */{} * * *", export_service.export_config.export_interval_hours),
        move |_uuid, _lock| {
            let service = export_service.clone();
            Box::pin(async move {
                match service.export_gottcha2().await {
                    Ok(metadata) => {
                        tracing::info!("GOTTCHA2 export completed: {:?}", metadata);
                        // Store metadata in database for API access
                        service.store_export_metadata(metadata).await;
                    }
                    Err(e) => {
                        tracing::error!("GOTTCHA2 export failed: {:?}", e);
                    }
                }
            })
        },
    )?;
    
    scheduler.add(gottcha2_job).await?;
    // Similar job for STAST
    
    scheduler.start().await?;
    Ok(())
}
```

#### REST API Endpoints

```rust
// src/handlers/export.rs

/// List available Parquet exports
#[utoipa::path(
    get,
    path = "/exports",
    tag = "export",
    params(
        ("table" = String, Query, description = "Table name (gottcha2 or stast)"),
        ("limit" = Option<u32>, Query, description = "Number of exports to return"),
    ),
    responses(
        (status = 200, body = Vec<ExportMetadata>, description = "List of available exports"),
    ),
)]
pub async fn list_exports(
    State(state): State<AppState>,
    Query(params): Query<ExportQuery>,
) -> Result<Json<Vec<ExportMetadata>>, AppError> {
    let exports = sqlx::query_as::<_, ExportMetadata>(
        "SELECT * FROM parquet_exports 
         WHERE table_name = $1 
         ORDER BY created_at DESC 
         LIMIT $2"
    )
    .bind(&params.table)
    .bind(params.limit.unwrap_or(10))
    .fetch_all(&state.db)
    .await?;
    
    Ok(Json(exports))
}

/// Download a specific Parquet export
#[utoipa::path(
    get,
    path = "/exports/{export_id}/download",
    tag = "export",
    responses(
        (status = 200, content_type = "application/octet-stream"),
        (status = 302, description = "Redirect to signed URL"),
        (status = 404, description = "Export not found"),
    ),
)]
pub async fn download_export(
    State(state): State<AppState>,
    Path(export_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let export = get_export_metadata(&state.db, export_id).await?;
    
    match state.storage_backend {
        StorageBackend::S3 { .. } => {
            // Generate pre-signed URL for direct S3 download
            let signed_url = generate_presigned_url(&export, Duration::hours(1)).await?;
            Ok(Redirect::temporary(&signed_url).into_response())
        }
        StorageBackend::LocalFS { .. } => {
            // Stream file directly
            let file = tokio::fs::File::open(&export.file_path).await?;
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);
            
            Ok(Response::builder()
                .header("Content-Type", "application/octet-stream")
                .header("Content-Disposition", format!("attachment; filename=\"{}\"", export.file_name))
                .body(body)
                .unwrap()
                .into_response())
        }
    }
}

/// Trigger manual export
#[utoipa::path(
    post,
    path = "/exports/trigger",
    tag = "export",
    request_body = ExportRequest,
    responses(
        (status = 202, body = ExportJob, description = "Export job started"),
    ),
)]
pub async fn trigger_export(
    State(state): State<AppState>,
    Json(request): Json<ExportRequest>,
) -> Result<Json<ExportJob>, AppError> {
    // Queue export job asynchronously
    let job_id = Uuid::new_v4();
    
    tokio::spawn(async move {
        let result = match request.table {
            Table::Gottcha2 => state.export_service.export_gottcha2().await,
            Table::Stast => state.export_service.export_stast().await,
        };
        // Update job status in database
    });
    
    Ok(Json(ExportJob {
        job_id,
        status: JobStatus::Pending,
        table: request.table,
        created_at: Utc::now(),
    }))
}
```

### Configuration Example

```toml
# In Cross.toml or environment
[export]
compression = "zstd"
compression_level = 3
batch_size = 100000
export_interval_hours = 24
retention_days = 90

[export.storage]
backend = "s3"
bucket = "nvd-exports"
prefix = "parquet/"
region = "us-east-1"
```

### Benefits

- **Efficient Storage**: Parquet with compression reduces storage by 70-90%
- **Fast Analytics**: Columnar format optimized for analytical queries
- **Schema Evolution**: Parquet handles schema changes gracefully
- **Streaming Support**: Can be streamed directly to clients
- **Cloud-Native**: Works with S3, Azure Blob, GCS
- **Incremental Exports**: Can export only new data since last export

### Performance Considerations

- Parquet files of 100MB-1GB are optimal for most use cases
- Zstd compression offers best compression/speed tradeoff
- Row group size of ~100k records balances memory usage and query performance
- Use Arrow's streaming API to handle large datasets without loading all into
  memory

### Client Usage Example

```python
# Python client example
import pandas as pd
import requests

# Get latest export
response = requests.get("https://api.nvd-support.org/exports?table=gottcha2&limit=1")
latest_export = response.json()[0]

# Download and load Parquet file
df = pd.read_parquet(latest_export["download_url"])

# Or use DuckDB for efficient SQL queries on Parquet
import duckdb
conn = duckdb.connect()
conn.execute(f"CREATE VIEW gottcha2 AS SELECT * FROM '{latest_export['download_url']}'")
result = conn.execute("SELECT * FROM gottcha2 WHERE abundance > 0.01").fetchdf()
```

---

## 3. LabKey LIMS Integration

### Overview

Integrate with LabKey Laboratory Information Management System (LIMS) to
automatically upload processed metagenomic data, eliminating the need for manual
handling of result files. This feature will use the upcoming Rust LabKey client
(similar to [labkey-api-python](https://github.com/LabKey/labkey-api-python)) to
authenticate and transmit data directly from the support car service to your
lab's LabKey server.

### Implementation Approach

#### Dependencies

```toml
# Cargo.toml additions (once the Rust client is available)
labkey-rs = "0.1"  # Placeholder for upcoming Rust LabKey client
backoff = { version = "0.4", features = ["tokio"] }
async-trait = "0.1"
```

#### LabKey Service Structure

```rust
// src/services/labkey.rs
use async_trait::async_trait;
use backoff::{ExponentialBackoff, future::retry};

#[derive(Clone)]
pub struct LabKeyService {
    client: LabKeyClient,
    config: LabKeyConfig,
    retry_policy: ExponentialBackoff,
}

#[derive(Clone, Deserialize)]
pub struct LabKeyConfig {
    pub server_url: String,
    pub project_path: String,
    pub schema_name: String,
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub batch_size: usize,
    pub auto_upload: bool,
    pub upload_interval_minutes: u32,
}

#[async_trait]
pub trait LabKeyUploader {
    async fn upload_gottcha2_batch(&self, records: Vec<Gottcha2Record>) -> Result<UploadResult, AppError>;
    async fn upload_stast_batch(&self, records: Vec<StastRecord>) -> Result<UploadResult, AppError>;
    async fn verify_connection(&self) -> Result<ConnectionStatus, AppError>;
}
```

#### Authentication Handler

```rust
impl LabKeyService {
    pub async fn new(config: LabKeyConfig) -> Result<Self, AppError> {
        // Initialize LabKey client with authentication
        let client = match (&config.api_key, &config.username, &config.password) {
            (Some(api_key), _, _) => {
                // Preferred: API key authentication
                LabKeyClient::with_api_key(&config.server_url, api_key)
                    .await?
            }
            (None, Some(username), Some(password)) => {
                // Fallback: Basic authentication
                LabKeyClient::with_credentials(&config.server_url, username, password)
                    .await?
            }
            _ => {
                return Err(AppError::Configuration(
                    "LabKey requires either API key or username/password".into()
                ));
            }
        };
        
        // Verify connection and permissions
        client.verify_project_access(&config.project_path).await?;
        
        Ok(Self {
            client,
            config,
            retry_policy: ExponentialBackoff::default(),
        })
    }
}
```

#### Data Upload Implementation

```rust
#[async_trait]
impl LabKeyUploader for LabKeyService {
    async fn upload_gottcha2_batch(
        &self, 
        records: Vec<Gottcha2Record>
    ) -> Result<UploadResult, AppError> {
        // Transform records to LabKey format
        let rows = records
            .into_iter()
            .map(|r| self.gottcha2_to_labkey_row(r))
            .collect::<Vec<_>>();
        
        // Upload with retry logic
        let result = retry(self.retry_policy.clone(), || async {
            self.client
                .insert_rows(
                    &self.config.project_path,
                    &self.config.schema_name,
                    "gottcha2_results",  // LabKey list name
                    &rows,
                )
                .await
                .map_err(backoff::Error::transient)
        })
        .await?;
        
        Ok(UploadResult {
            rows_uploaded: result.row_count,
            labkey_transaction_id: result.transaction_id,
            timestamp: Utc::now(),
        })
    }
    
    fn gottcha2_to_labkey_row(&self, record: Gottcha2Record) -> LabKeyRow {
        // Map Rust struct to LabKey's expected format
        let mut row = LabKeyRow::new();
        
        // Standard LabKey fields
        row.insert("SampleId", record.sample_id);
        row.insert("RunId", record.run_id);
        row.insert("AnalysisDate", record.created_at.to_rfc3339());
        
        // GOTTCHA2-specific fields
        row.insert("Rollup", record.rollup);
        row.insert("Taxonomy", record.taxonomy);
        row.insert("TaxonomyId", record.taxonomy_id);
        row.insert("LinearLength", record.linear_length);
        row.insert("TotalBpMapped", record.total_bp_mapped);
        row.insert("Abundance", record.linear_abundance);
        row.insert("DoC", record.linear_doc);
        
        // Metadata
        row.insert("UploadedBy", "nvd-support-car");
        row.insert("UploadTimestamp", Utc::now().to_rfc3339());
        
        row
    }
}
```

#### Automatic Upload Pipeline

```rust
// src/handlers/labkey_sync.rs
pub struct LabKeySyncHandler {
    labkey_service: Arc<LabKeyService>,
    db: PgPool,
}

impl LabKeySyncHandler {
    /// Process new records that haven't been uploaded to LabKey
    pub async fn sync_pending_records(&self) -> Result<SyncReport, AppError> {
        let mut report = SyncReport::default();
        
        // Get records not yet uploaded
        let pending_gottcha2 = sqlx::query_as::<_, Gottcha2Record>(
            "SELECT * FROM gottcha2_full 
             WHERE labkey_upload_status IS NULL 
             OR labkey_upload_status = 'pending'
             ORDER BY created_at
             LIMIT $1"
        )
        .bind(self.labkey_service.config.batch_size as i64)
        .fetch_all(&self.db)
        .await?;
        
        if !pending_gottcha2.is_empty() {
            match self.labkey_service.upload_gottcha2_batch(pending_gottcha2.clone()).await {
                Ok(result) => {
                    // Mark records as uploaded
                    self.mark_as_uploaded(
                        "gottcha2_full",
                        &pending_gottcha2.iter().map(|r| r.id).collect::<Vec<_>>(),
                        &result.labkey_transaction_id,
                    ).await?;
                    
                    report.gottcha2_uploaded = result.rows_uploaded;
                }
                Err(e) => {
                    report.errors.push(format!("GOTTCHA2 upload failed: {:?}", e));
                    // Records remain in pending state for retry
                }
            }
        }
        
        // Similar logic for STAST records...
        
        Ok(report)
    }
}
```

#### Scheduled Sync Job

```rust
// src/jobs/labkey_scheduler.rs
pub async fn setup_labkey_sync_scheduler(
    sync_handler: Arc<LabKeySyncHandler>,
    interval_minutes: u32,
) -> Result<(), AppError> {
    let scheduler = JobScheduler::new().await?;
    
    let sync_job = Job::new_async(
        format!("0 */{} * * * *", interval_minutes),
        move |_uuid, _lock| {
            let handler = sync_handler.clone();
            Box::pin(async move {
                match handler.sync_pending_records().await {
                    Ok(report) => {
                        tracing::info!("LabKey sync completed: {:?}", report);
                        
                        // Emit metrics
                        metrics::counter!("labkey_records_uploaded")
                            .increment(report.total_uploaded() as u64);
                    }
                    Err(e) => {
                        tracing::error!("LabKey sync failed: {:?}", e);
                        metrics::counter!("labkey_sync_errors").increment(1);
                    }
                }
            })
        },
    )?;
    
    scheduler.add(sync_job).await?;
    scheduler.start().await?;
    
    Ok(())
}
```

#### REST API for Manual Control

```rust
// src/handlers/labkey_api.rs

/// Manually trigger LabKey synchronization
#[utoipa::path(
    post,
    path = "/labkey/sync",
    tag = "labkey",
    responses(
        (status = 200, body = SyncReport, description = "Sync completed"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Sync failed"),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn trigger_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SyncReport>, AppError> {
    validate_bearer_token(&state, &headers)?;
    
    let report = state.labkey_sync_handler.sync_pending_records().await?;
    Ok(Json(report))
}

/// Check LabKey connection status
#[utoipa::path(
    get,
    path = "/labkey/status",
    tag = "labkey",
    responses(
        (status = 200, body = ConnectionStatus, description = "Connection status"),
    ),
)]
pub async fn check_status(
    State(state): State<AppState>,
) -> Result<Json<ConnectionStatus>, AppError> {
    let status = state.labkey_service.verify_connection().await?;
    Ok(Json(status))
}

/// Get sync statistics
#[utoipa::path(
    get,
    path = "/labkey/stats",
    tag = "labkey",
    responses(
        (status = 200, body = SyncStatistics, description = "Sync statistics"),
    ),
)]
pub async fn get_sync_stats(
    State(state): State<AppState>,
) -> Result<Json<SyncStatistics>, AppError> {
    let stats = sqlx::query_as::<_, SyncStatistics>(
        "SELECT 
            COUNT(*) FILTER (WHERE labkey_upload_status = 'completed') as uploaded_count,
            COUNT(*) FILTER (WHERE labkey_upload_status = 'pending' OR labkey_upload_status IS NULL) as pending_count,
            COUNT(*) FILTER (WHERE labkey_upload_status = 'failed') as failed_count,
            MAX(labkey_upload_timestamp) as last_sync_time
         FROM gottcha2_full"
    )
    .fetch_one(&state.db)
    .await?;
    
    Ok(Json(stats))
}
```

### Database Schema Updates

```sql
-- Migration: 004_labkey_sync_tracking.sql
ALTER TABLE gottcha2_full ADD COLUMN IF NOT EXISTS 
    labkey_upload_status VARCHAR(20) DEFAULT 'pending' 
    CHECK (labkey_upload_status IN ('pending', 'completed', 'failed'));

ALTER TABLE gottcha2_full ADD COLUMN IF NOT EXISTS 
    labkey_upload_timestamp TIMESTAMPTZ;

ALTER TABLE gottcha2_full ADD COLUMN IF NOT EXISTS 
    labkey_transaction_id VARCHAR(255);

-- Similar for stast_table
ALTER TABLE stast_table ADD COLUMN IF NOT EXISTS 
    labkey_upload_status VARCHAR(20) DEFAULT 'pending';
ALTER TABLE stast_table ADD COLUMN IF NOT EXISTS 
    labkey_upload_timestamp TIMESTAMPTZ;
ALTER TABLE stast_table ADD COLUMN IF NOT EXISTS 
    labkey_transaction_id VARCHAR(255);

-- Index for efficient queries on pending records
CREATE INDEX idx_gottcha2_labkey_status ON gottcha2_full(labkey_upload_status) 
    WHERE labkey_upload_status = 'pending';
CREATE INDEX idx_stast_labkey_status ON stast_table(labkey_upload_status) 
    WHERE labkey_upload_status = 'pending';
```

### Configuration Example

```toml
# In .env or environment variables
LABKEY_SERVER_URL="https://labkey.yourlab.org"
LABKEY_PROJECT_PATH="/NVD/Metagenomics"
LABKEY_SCHEMA_NAME="lists"
LABKEY_API_KEY="your-api-key-here"
LABKEY_BATCH_SIZE=1000
LABKEY_AUTO_UPLOAD=true
LABKEY_UPLOAD_INTERVAL_MINUTES=15
```

### Benefits

- **Eliminate Manual File Handling**: No more results/ directory with text files
  to manually process
- **Real-time Data Availability**: Results available in LabKey within minutes of
  processing
- **Automatic Retry Logic**: Failed uploads are retried with exponential backoff
- **Audit Trail**: Complete tracking of what was uploaded when
- **Bulk Upload Efficiency**: Batch uploads reduce API calls and improve
  throughput
- **Secure Authentication**: Supports both API key and credential-based auth
- **Monitoring & Metrics**: Track upload success rates and latencies
- **Idempotent Operations**: Safe to retry without duplicating data

### Workflow Transformation

**Before (Manual Process):**

```
HTCondor → NVD Pipeline → Text Files → Manual Upload → LabKey
           (results/*.txt)    ↑
                         (Human Intervention)
```

**After (Automated):**

```
HTCondor → NVD Pipeline → Support Car → LabKey
                          (Automatic)
```

### Integration with Existing Features

This LabKey integration works seamlessly with the other roadmap features:

- **OpenAPI**: LabKey sync endpoints are documented in the API spec
- **Parquet Export**: Can export both raw data AND LabKey upload status
- **Monitoring**: Upload metrics integrated into overall system health

---

## Implementation Priority

### Phase 1: OpenAPI Specification (Week 1-2)

- Add utoipa dependencies
- Document existing endpoints
- Deploy interactive documentation
- Generate TypeScript/Python clients

### Phase 2: Basic Parquet Export (Week 3-4)

- Implement manual export endpoints
- Add Arrow/Parquet conversion
- Local filesystem storage
- Basic compression (Snappy)

### Phase 3: Scheduled Exports (Week 5)

- Add cron scheduler
- Implement retention policies
- Export metadata tracking
- Monitoring/alerting

### Phase 4: Cloud Storage (Week 6)

- S3/Azure/GCS integration
- Pre-signed URLs
- CDN distribution
- Incremental exports

## Testing Strategy

### OpenAPI Testing

- Validate spec against actual endpoints
- Test client SDK generation
- Ensure examples work

### Parquet Export Testing

- Unit tests for Arrow conversion
- Integration tests with test database
- Performance tests with large datasets
- Compression ratio benchmarks
- Client compatibility tests (Python, R, DuckDB)
