use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub patient_id: String,
    pub filename: String,
    pub vault_path: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub created_at: String,
}
