//! Save conflict comparison and user resolution choices for swap/restore.

use serde::{Deserialize, Serialize};

use crate::domain::save::SaveMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SaveFreshness {
    SourceNewer,
    DestinationNewer,
    Equal,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResolutionChoice {
    KeepSource,
    KeepDestination,
    KeepBothRename,
    CancelOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictComparison {
    pub source_path: String,
    pub destination_path: String,
    pub source_metadata: SaveMetadata,
    pub destination_metadata: SaveMetadata,
    pub freshness: SaveFreshness,
    pub reason: String,
}
