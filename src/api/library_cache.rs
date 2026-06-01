use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::dto::LibraryStateDto;
use crate::platform::fs::ensure_directory;

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedLibraryCache {
    version: u32,
    saved_at: DateTime<Utc>,
    games: LibraryStateDto,
}

pub fn cache_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("library-cache.json");
    }
    PathBuf::from("slotforge-library-cache.json")
}

pub fn save(library: &LibraryStateDto) -> Result<()> {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let payload = PersistedLibraryCache {
        version: CACHE_VERSION,
        saved_at: Utc::now(),
        games: LibraryStateDto {
            // Clone library fields; omit last_swap so rollback state is not persisted in cache.
            games: library.games.clone(),
            vault_by_game_id: library.vault_by_game_id.clone(),
            last_swap: None,
        },
    };

    let json = serde_json::to_string_pretty(&payload).context("failed to serialize library cache")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write library cache at {}", path.display()))?;
    Ok(())
}

pub fn load() -> Result<Option<LibraryStateDto>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read library cache at {}", path.display()))?;
    let cached = serde_json::from_str::<PersistedLibraryCache>(&raw).with_context(|| {
        format!(
            "failed to parse library cache JSON at {}",
            path.display()
        )
    })?;

    if cached.version != CACHE_VERSION {
        return Ok(None);
    }

    Ok(Some(cached.games))
}
