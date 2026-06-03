use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::domain::game::{GameRecord, GameSource};
use crate::platform::fs::{ensure_directory, resolve_path, validate_directory};
use crate::services::blacklist_service;
use crate::services::discovery_service::DiscoverySummary;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ManualGameRegistry {
    games: Vec<GameRecord>,
}

pub fn list_manual_games() -> Result<Vec<GameRecord>> {
    let mut registry = load_registry()?;
    registry.games.sort_by_key(|a| a.name.to_lowercase());
    Ok(registry.games)
}

pub fn build_canonical_library(discovery: DiscoverySummary) -> Result<Vec<GameRecord>> {
    let manual_games = list_manual_games()?;
    let mut by_identity = HashMap::<String, GameRecord>::new();

    for game in discovery.discovered_games {
        let key = identity_key(&game.active_save_dir);
        by_identity.insert(key, game);
    }

    // Manual entries win on conflicts to preserve user intent.
    for manual in manual_games {
        let key = identity_key(&manual.active_save_dir);
        if let Some(auto) = by_identity.get_mut(&key) {
            auto.name = manual.name.clone();
            auto.id = manual.id.clone();
            auto.source = GameSource::UserAdded;
            auto.updated_at = manual.updated_at;
            if !auto.tags.iter().any(|tag| tag == "manual-entry") {
                auto.tags.push("manual-entry".to_string());
            }
            continue;
        }
        by_identity.insert(key, manual);
    }

    let mut merged: Vec<GameRecord> = by_identity.into_values().collect();
    merged.sort_by_key(|a| a.name.to_lowercase());
    blacklist_service::filter_games(merged)
}

pub fn add_manual_game(name: &str, raw_save_dir: &str) -> Result<GameRecord> {
    let cleaned_name = name.trim();
    if cleaned_name.is_empty() {
        anyhow::bail!("manual game name cannot be empty");
    }

    let save_dir = resolve_path(raw_save_dir);
    if save_dir.as_os_str().is_empty() {
        anyhow::bail!("manual save path cannot be empty");
    }
    validate_directory(&save_dir)?;

    let now = Utc::now();
    let id = format!(
        "manual:{}:{}",
        cleaned_name.to_lowercase(),
        save_dir.to_string_lossy().to_lowercase()
    );

    let mut registry = load_registry()?;
    if let Some(existing) = registry.games.iter_mut().find(|game| game.id == id) {
        existing.name = cleaned_name.to_string();
        existing.active_save_dir = save_dir.clone();
        existing.game_root = save_dir.parent().map(|value| value.to_path_buf());
        existing.updated_at = now;
        let updated = existing.clone();
        save_registry(&registry)?;
        return Ok(updated);
    }

    let record = GameRecord {
        id,
        name: cleaned_name.to_string(),
        game_root: save_dir.parent().map(|value| value.to_path_buf()),
        active_save_dir: save_dir,
        source: GameSource::UserAdded,
        tags: vec!["manual-entry".to_string()],
        created_at: now,
        updated_at: now,
    };
    registry.games.push(record.clone());
    save_registry(&registry)?;
    Ok(record)
}

pub fn remove_manual_game(game_id: &str) -> Result<()> {
    let mut registry = load_registry()?;
    registry.games.retain(|game| game.id != game_id);
    save_registry(&registry)?;
    Ok(())
}

pub fn edit_manual_game(
    game_id: &str,
    new_name: &str,
    new_raw_save_dir: &str,
) -> Result<GameRecord> {
    let cleaned_name = new_name.trim();
    if cleaned_name.is_empty() {
        anyhow::bail!("manual game name cannot be empty");
    }

    let save_dir = resolve_path(new_raw_save_dir);
    if save_dir.as_os_str().is_empty() {
        anyhow::bail!("manual save path cannot be empty");
    }
    validate_directory(&save_dir)?;

    let mut registry = load_registry()?;
    let Some(existing) = registry.games.iter_mut().find(|game| game.id == game_id) else {
        anyhow::bail!("manual game with id '{game_id}' was not found");
    };

    existing.name = cleaned_name.to_string();
    existing.active_save_dir = save_dir.clone();
    existing.game_root = save_dir.parent().map(|value| value.to_path_buf());
    existing.updated_at = Utc::now();
    let updated = existing.clone();

    save_registry(&registry)?;
    Ok(updated)
}

fn load_registry() -> Result<ManualGameRegistry> {
    let path = registry_path();
    if !path.exists() {
        return Ok(ManualGameRegistry::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read manual game registry at {}", path.display()))?;
    let registry = serde_json::from_str::<ManualGameRegistry>(&raw).with_context(|| {
        format!(
            "failed to parse manual game registry JSON at {}",
            path.display()
        )
    })?;
    Ok(registry)
}

fn save_registry(registry: &ManualGameRegistry) -> Result<()> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize manual game registry")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write manual game registry at {}", path.display()))?;
    Ok(())
}

fn registry_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("manual-games.json");
    }
    PathBuf::from("slotforge-manual-games.json")
}

fn identity_key(path: &std::path::Path) -> String {
    identity_key_for_path(path)
}

/// Stable key for matching games by save directory (used by API facade).
pub fn identity_key_for_path(path: &std::path::Path) -> String {
    crate::platform::fs::normalize_path(path)
        .to_string_lossy()
        .to_lowercase()
}
