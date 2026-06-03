use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::game::GameRecord;
use crate::platform::fs::{ensure_directory, normalize_path, resolve_path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IgnoredEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub name: Option<String>,
    pub ignored_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct IgnoredRegistry {
    entries: Vec<IgnoredEntry>,
}

pub fn list_ignored() -> Result<Vec<IgnoredEntry>> {
    let registry = load_registry()?;
    let mut entries = registry.entries;
    entries.sort_by(|a, b| {
        let name_a = display_name(a).to_lowercase();
        let name_b = display_name(b).to_lowercase();
        name_a.cmp(&name_b)
    });
    Ok(entries)
}

pub fn add_ignored_path(raw_path: &str, name: Option<String>) -> Result<IgnoredEntry> {
    let path = resolve_path(raw_path.trim());
    if path.as_os_str().is_empty() {
        anyhow::bail!("ignored path cannot be empty");
    }

    let normalized = normalize_path(&path);

    let cleaned_name = name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut registry = load_registry()?;
    let key = path_key(&normalized);
    if let Some(existing) = registry
        .entries
        .iter_mut()
        .find(|entry| path_key(&entry.path) == key)
    {
        if cleaned_name.is_some() {
            existing.name = cleaned_name;
        }
        existing.ignored_at = Utc::now();
        let updated = existing.clone();
        save_registry(&registry)?;
        return Ok(updated);
    }

    let entry = IgnoredEntry {
        path: normalized,
        name: cleaned_name,
        ignored_at: Utc::now(),
    };
    registry.entries.push(entry.clone());
    save_registry(&registry)?;
    Ok(entry)
}

pub fn remove_ignored_path(raw_path: &str) -> Result<()> {
    let path = resolve_path(raw_path.trim());
    let key = path_key(&path);
    let mut registry = load_registry()?;
    let before = registry.entries.len();
    registry
        .entries
        .retain(|entry| path_key(&entry.path) != key);
    if registry.entries.len() == before {
        anyhow::bail!("ignored path was not found");
    }
    save_registry(&registry)?;
    Ok(())
}

pub fn ignore_game(game: &GameRecord) -> Result<IgnoredEntry> {
    add_ignored_path(
        &game.active_save_dir.to_string_lossy(),
        Some(game.name.clone()),
    )
}

pub fn is_path_ignored(path: &Path) -> Result<bool> {
    let normalized = normalize_path(path);
    let entries = list_ignored()?;
    Ok(entries
        .iter()
        .any(|entry| paths_overlap(&normalized, &entry.path)))
}

pub fn filter_games(games: Vec<GameRecord>) -> Result<Vec<GameRecord>> {
    let entries = list_ignored()?;
    if entries.is_empty() {
        return Ok(games);
    }

    Ok(games
        .into_iter()
        .filter(|game| !is_game_ignored(game, &entries))
        .collect())
}

fn is_game_ignored(game: &GameRecord, entries: &[IgnoredEntry]) -> bool {
    if entries
        .iter()
        .any(|entry| paths_overlap(&game.active_save_dir, &entry.path))
    {
        return true;
    }

    if let Some(root) = &game.game_root {
        if entries.iter().any(|entry| paths_overlap(root, &entry.path)) {
            return true;
        }
    }

    false
}

/// True when `candidate` equals `ignored` or lies under it (prefix by path components).
pub fn paths_overlap(candidate: &Path, ignored: &Path) -> bool {
    let candidate = normalize_path(candidate);
    let ignored = normalize_path(ignored);
    candidate == ignored || candidate.starts_with(&ignored)
}

fn display_name(entry: &IgnoredEntry) -> String {
    entry.name.clone().unwrap_or_else(|| {
        entry
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.path.display().to_string())
    })
}

fn path_key(path: &Path) -> String {
    normalize_path(path).to_string_lossy().to_lowercase()
}

fn load_registry() -> Result<IgnoredRegistry> {
    let path = registry_path();
    if !path.exists() {
        return Ok(IgnoredRegistry::default());
    }

    let raw = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read ignored games registry at {}",
            path.display()
        )
    })?;
    let registry = serde_json::from_str::<IgnoredRegistry>(&raw).with_context(|| {
        format!(
            "failed to parse ignored games registry JSON at {}",
            path.display()
        )
    })?;
    Ok(registry)
}

fn save_registry(registry: &IgnoredRegistry) -> Result<()> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize ignored games registry")?;
    fs::write(&path, json).with_context(|| {
        format!(
            "failed to write ignored games registry at {}",
            path.display()
        )
    })?;
    Ok(())
}

fn registry_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("ignored-games.json");
    }
    PathBuf::from("slotforge-ignored-games.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_overlap_exact_and_child() {
        #[cfg(windows)]
        let (parent, child, sibling) = (
            PathBuf::from(r"C:\Games\Cyberpunk"),
            PathBuf::from(r"C:\Games\Cyberpunk\Saves\slot1"),
            PathBuf::from(r"C:\Games\Cyberpunk2077"),
        );
        #[cfg(not(windows))]
        let (parent, child, sibling) = (
            PathBuf::from("/games/cyberpunk"),
            PathBuf::from("/games/cyberpunk/saves/slot1"),
            PathBuf::from("/games/cyberpunk2077"),
        );

        assert!(paths_overlap(&parent, &parent));
        assert!(paths_overlap(&child, &parent));
        assert!(!paths_overlap(&sibling, &parent));
    }
}
