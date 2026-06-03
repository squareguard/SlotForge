use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::conflict::{ConflictComparison, SaveFreshness};
use crate::domain::game::{GameRecord, GameSource};
use crate::domain::save::{SaveMetadata, SaveOrigin, SaveRecord};
use crate::services::discovery_service::DiscoveredSaveFile;

const LABEL_COLORS: [&str; 6] = [
    "#00f5ff", "#ffb800", "#a855f7", "#22c55e", "#ff2d55", "#f472b6",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveMetadataDto {
    pub modified_at: Option<String>,
    pub created_at: Option<String>,
    pub byte_size: u64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFileDto {
    pub path: String,
    pub freshness: SaveFreshness,
    pub active_snippet: String,
    pub snapshot_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GameDto {
    pub id: String,
    pub name: String,
    pub game_root: Option<String>,
    pub active_save_dir: String,
    pub source: GameSource,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_backed_up_at: Option<String>,
    pub has_conflict: bool,
    pub conflict_files: Vec<ConflictFileDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IntegrityStatusDto {
    Verified,
    Corrupted,
    Unchecked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    pub id: String,
    pub game_id: String,
    pub file_name: String,
    pub absolute_path: String,
    pub origin: SaveOrigin,
    pub label: Option<String>,
    pub note: Option<String>,
    pub metadata: SaveMetadataDto,
    pub archived_at: Option<String>,
    pub integrity: IntegrityStatusDto,
    pub label_color: String,
    pub file_count: u32,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LastSwapDto {
    pub game_id: String,
    pub snapshot_id: String,
    pub previous_active_path: String,
    pub restored_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStateDto {
    pub games: Vec<GameDto>,
    pub vault_by_game_id: HashMap<String, Vec<SnapshotDto>>,
    pub last_swap: Option<LastSwapDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AddGameResultDto {
    pub library: LibraryStateDto,
    pub game: GameDto,
    pub discovered_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupResultDto {
    pub library: LibraryStateDto,
    pub snapshot: SnapshotDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResultDto {
    pub library: LibraryStateDto,
    pub last_swap: LastSwapDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotResultDto {
    pub library: LibraryStateDto,
    pub snapshot: SnapshotDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VerifyAllResultDto {
    pub library: LibraryStateDto,
    pub verified_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredEntryDto {
    pub path: String,
    pub name: Option<String>,
    pub ignored_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredListDto {
    pub entries: Vec<IgnoredEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreGameResultDto {
    pub library: LibraryStateDto,
    pub entry: IgnoredEntryDto,
}

pub fn metadata_to_dto(metadata: &SaveMetadata) -> SaveMetadataDto {
    SaveMetadataDto {
        modified_at: metadata.modified_at.map(|t| t.to_rfc3339()),
        created_at: metadata.created_at.map(|t| t.to_rfc3339()),
        byte_size: metadata.byte_size,
        sha256: metadata.sha256.clone(),
    }
}

pub fn label_color_for_id(id: &str) -> String {
    let hash = id
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(u32::from(b)));
    LABEL_COLORS[(hash as usize) % LABEL_COLORS.len()].to_string()
}

pub fn integrity_from_verified(verified: Option<bool>) -> IntegrityStatusDto {
    match verified {
        Some(true) => IntegrityStatusDto::Verified,
        Some(false) => IntegrityStatusDto::Corrupted,
        None => IntegrityStatusDto::Unchecked,
    }
}

pub fn snapshot_to_dto(
    record: &SaveRecord,
    integrity: IntegrityStatusDto,
    files: Vec<String>,
) -> SnapshotDto {
    SnapshotDto {
        id: record.id.clone(),
        game_id: record.game_id.clone(),
        file_name: record.file_name.clone(),
        absolute_path: path_to_string(&record.absolute_path),
        origin: record.origin.clone(),
        label: record.label.clone(),
        note: record.note.clone(),
        metadata: metadata_to_dto(&record.metadata),
        archived_at: record.archived_at.map(|t| t.to_rfc3339()),
        integrity,
        label_color: record
            .label_color
            .clone()
            .unwrap_or_else(|| label_color_for_id(&record.id)),
        file_count: files.len() as u32,
        files,
    }
}

pub fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn conflict_file_from_comparison(
    file_name: &str,
    comparison: &ConflictComparison,
) -> ConflictFileDto {
    ConflictFileDto {
        path: file_name.to_string(),
        freshness: comparison.freshness.clone(),
        active_snippet: format!(
            "ACTIVE  path={}  {}",
            comparison.destination_path, comparison.reason
        ),
        snapshot_snippet: format!(
            "VAULT   path={}  freshness={:?}",
            comparison.source_path, comparison.freshness
        ),
    }
}

pub fn game_to_dto(
    game: &GameRecord,
    last_backed_up_at: Option<DateTime<Utc>>,
    has_conflict: bool,
    conflict_files: Vec<ConflictFileDto>,
) -> GameDto {
    GameDto {
        id: game.id.clone(),
        name: game.name.clone(),
        game_root: game.game_root.as_ref().map(|p| path_to_string(p.as_path())),
        active_save_dir: path_to_string(&game.active_save_dir),
        source: game.source.clone(),
        tags: game.tags.clone(),
        created_at: game.created_at.to_rfc3339(),
        updated_at: game.updated_at.to_rfc3339(),
        last_backed_up_at: last_backed_up_at.map(|t| t.to_rfc3339()),
        has_conflict,
        conflict_files,
    }
}

pub fn discovered_to_relative_paths(files: &[DiscoveredSaveFile]) -> Vec<String> {
    files.iter().map(|f| f.relative_path.clone()).collect()
}

pub fn ignored_entry_to_dto(
    entry: &crate::services::blacklist_service::IgnoredEntry,
) -> IgnoredEntryDto {
    IgnoredEntryDto {
        path: path_to_string(&entry.path),
        name: entry.name.clone(),
        ignored_at: entry.ignored_at.to_rfc3339(),
    }
}
