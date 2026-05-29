use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameSource {
    AutoDiscovered,
    UserAdded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameRecord {
    pub id: String,
    pub name: String,
    pub game_root: Option<PathBuf>,
    pub active_save_dir: PathBuf,
    pub source: GameSource,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
