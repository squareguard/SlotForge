use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::services::config_service;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LastSwapSession {
    pub game_id: String,
    pub snapshot_id: String,
    pub active_destination_path: PathBuf,
    pub staged_active_backup_path: Option<PathBuf>,
    pub restored_at: DateTime<Utc>,
}

fn session_path() -> Result<PathBuf> {
    let _ = config_service::ensure_initialized()?;
    let config_path = config_service::config_path();
    let parent = config_path
        .parent()
        .context("config path has no parent directory")?;
    Ok(parent.join("last-swap.json"))
}

pub fn load_last_swap() -> Result<Option<LastSwapSession>> {
    let _ = crate::services::config_service::ensure_initialized()?;
    let path = session_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read last swap session at {}", path.display()))?;
    let session: LastSwapSession = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse last swap session at {}", path.display()))?;
    Ok(Some(session))
}

pub fn save_last_swap(session: &LastSwapSession) -> Result<()> {
    let path = session_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create directory for last swap session at {}",
                parent.display()
            )
        })?;
    }
    let raw =
        serde_json::to_string_pretty(session).context("failed to serialize last swap session")?;
    fs::write(&path, raw)
        .with_context(|| format!("failed to write last swap session at {}", path.display()))?;
    Ok(())
}

pub fn clear_last_swap() -> Result<()> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove last swap session at {}", path.display()))?;
    }
    Ok(())
}

pub fn session_to_dto(session: &LastSwapSession) -> crate::api::dto::LastSwapDto {
    crate::api::dto::LastSwapDto {
        game_id: session.game_id.clone(),
        snapshot_id: session.snapshot_id.clone(),
        previous_active_path: session
            .staged_active_backup_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                session
                    .active_destination_path
                    .to_string_lossy()
                    .to_string()
            }),
        restored_at: session.restored_at.to_rfc3339(),
    }
}

pub fn active_destination(session: &LastSwapSession) -> &Path {
    &session.active_destination_path
}

pub fn staged_backup(session: &LastSwapSession) -> Option<&Path> {
    session.staged_active_backup_path.as_deref()
}
