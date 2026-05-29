use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SaveOrigin {
    ActiveDirectory,
    Vault,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveMetadata {
    pub modified_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub byte_size: u64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveRecord {
    pub id: String,
    pub game_id: String,
    pub file_name: String,
    pub absolute_path: PathBuf,
    pub origin: SaveOrigin,
    pub label: Option<String>,
    pub note: Option<String>,
    /// User-chosen tag colour from annotations; when `None`, UI derives a default from `id`.
    pub label_color: Option<String>,
    pub metadata: SaveMetadata,
    pub archived_at: Option<DateTime<Utc>>,
}
