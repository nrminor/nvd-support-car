use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DummyRecord {
    pub run_id: String,
    pub task_id: String,
    pub shard: i32,
    pub idempotency_key: String,
    pub schema_version: i32,
    pub payload: serde_json::Value,
}
