use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::domain::game::{GameRecord, GameSource};
use crate::platform::fs::{normalize_and_dedup_paths, walk_tree};
use crate::platform::path_defaults::default_scan_paths;
use crate::services::blacklist_service;
use crate::services::config_service;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverySummary {
    pub scanned_roots: Vec<PathBuf>,
    pub discovered_games: Vec<GameRecord>,
}

pub fn discover_from_default_locations() -> Result<DiscoverySummary> {
    let roots = default_scan_paths();
    discover_games_from_roots(&roots)
}

pub fn discover_from_configured_locations() -> Result<DiscoverySummary> {
    let roots = config_service::get_scan_paths()?;
    discover_games_from_roots(&roots)
}

pub fn discover_and_merge_library() -> Result<Vec<GameRecord>> {
    let summary = discover_from_configured_locations()?;
    crate::services::library_service::build_canonical_library(summary)
}

pub fn discover_games_from_roots(roots: &[PathBuf]) -> Result<DiscoverySummary> {
    let normalized_roots = normalize_and_dedup_paths(roots.to_vec());
    let mut discovered = HashMap::<String, GameRecord>::new();

    for root in &normalized_roots {
        if !root.exists() || !root.is_dir() {
            continue;
        }
        if blacklist_service::is_path_ignored(root)? {
            continue;
        }

        for game_dir in collect_candidate_game_dirs(root)? {
            if blacklist_service::is_path_ignored(&game_dir)? {
                continue;
            }
            if let Some(record) = game_record_from_dir(&game_dir) {
                if blacklist_service::is_path_ignored(&record.active_save_dir)? {
                    continue;
                }
                discovered.entry(record.id.clone()).or_insert(record);
            }
        }
    }

    let mut discovered_games: Vec<GameRecord> = discovered.into_values().collect();
    discovered_games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(DiscoverySummary {
        scanned_roots: normalized_roots,
        discovered_games,
    })
}

/// Names of per-slot save folders (e.g. Cyberpunk's `AutoSave-1`) — not separate games.
fn is_save_slot_directory_name(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower == "saves" || lower == "save" || lower == "savegames" || lower == "save games" {
        return true;
    }
    if lower.starts_with("autosave")
        || lower.starts_with("quicksave")
        || lower.starts_with("manualsave")
        || lower.starts_with("cloudsave")
        || lower.starts_with("savegame")
        || lower.starts_with("save_")
        || lower.starts_with("slot")
    {
        return true;
    }
    // Steam-style numeric user dirs under a game folder (e.g. 76561198000000000)
    lower.chars().all(|c| c.is_ascii_digit()) && lower.len() >= 8
}

/// Walk up from a save's parent folder to the game root (e.g. `.../Cyberpunk 2077`, not `AutoSave-1`).
fn canonical_game_root(mut dir: PathBuf, scan_root: &Path) -> PathBuf {
    loop {
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            break;
        };
        if !is_save_slot_directory_name(name) {
            break;
        }
        let Some(parent) = dir.parent() else {
            break;
        };
        if parent == scan_root {
            break;
        }
        if !parent.starts_with(scan_root) {
            break;
        }
        dir = parent.to_path_buf();
    }
    dir
}

fn collect_candidate_game_dirs(root: &Path) -> Result<Vec<PathBuf>> {
    let mut game_roots = HashMap::<String, PathBuf>::new();

    for path in walk_tree(root, 5)? {
        if !path.is_file() || !is_save_like_extension(&path) {
            continue;
        }
        let Some(save_parent) = path.parent() else {
            continue;
        };
        let game_root = canonical_game_root(save_parent.to_path_buf(), root);
        let key = game_root.to_string_lossy().to_lowercase();
        game_roots.entry(key).or_insert(game_root);
    }

    Ok(normalize_and_dedup_paths(game_roots.into_values().collect()))
}

/// Save-like file discovered under a user-selected directory (for add-game / rescan UI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSaveFile {
    pub name: String,
    pub absolute_path: String,
    pub relative_path: String,
    pub size: u64,
    pub modified_at: String,
}

/// Walk `root` up to depth 4 and return save-like files (same extensions as discovery).
pub fn scan_save_files_in_directory(root: &Path) -> Result<Vec<DiscoveredSaveFile>> {
    let mut files = Vec::new();
    if !root.exists() || !root.is_dir() {
        return Ok(files);
    }

    for path in walk_tree(root, 4)? {
        if !path.is_file() || !is_save_like_extension(&path) {
            continue;
        }

        let metadata = fs::metadata(&path)?;
        let modified_at = metadata
            .modified()
            .map(chrono::DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());

        let relative_path = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });

        files.push(DiscoveredSaveFile {
            name: path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "save.dat".to_string()),
            absolute_path: path.to_string_lossy().to_string(),
            relative_path,
            size: metadata.len(),
            modified_at: modified_at.to_rfc3339(),
        });
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

fn is_save_like_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "sav" | "save" | "dat" | "bak" | "profile" | "json"
            )
        })
        .unwrap_or(false)
}

fn game_record_from_dir(path: &Path) -> Option<GameRecord> {
    let name = path.file_name()?.to_string_lossy().trim().to_string();
    if name.is_empty() {
        return None;
    }

    let now = Utc::now();
    Some(GameRecord {
        id: format!("auto:{}", path.to_string_lossy().to_lowercase()),
        name,
        game_root: path.parent().map(|value| value.to_path_buf()),
        active_save_dir: path.to_path_buf(),
        source: GameSource::AutoDiscovered,
        tags: vec!["auto-discovered".to_string()],
        created_at: now,
        updated_at: now,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::discover_games_from_roots;

    #[test]
    fn discovers_save_directories_from_root() {
        let temp_root = unique_temp_dir("discovery");
        let game_dir = temp_root.join("MyGame");
        fs::create_dir_all(&game_dir).expect("create game dir");
        fs::write(game_dir.join("slot1.sav"), b"save-data").expect("write save file");

        let summary = discover_games_from_roots(std::slice::from_ref(&temp_root))
            .expect("discover games should succeed");

        assert_eq!(summary.scanned_roots.len(), 1);
        assert_eq!(summary.discovered_games.len(), 1);
        assert_eq!(summary.discovered_games[0].name, "MyGame");
        assert_eq!(summary.discovered_games[0].active_save_dir, game_dir);

        fs::remove_dir_all(temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn groups_nested_autosave_slots_under_game_folder() {
        let temp_root = unique_temp_dir("discovery_nested");
        let publisher = temp_root.join("CD Projekt Red");
        let game_dir = publisher.join("Cyberpunk 2077");
        for slot in ["AutoSave-1", "AutoSave-2", "AutoSave-3"] {
            let slot_dir = game_dir.join(slot);
            fs::create_dir_all(&slot_dir).expect("create slot dir");
            fs::write(slot_dir.join("sav.dat"), b"save-data").expect("write save file");
        }

        let summary = discover_games_from_roots(std::slice::from_ref(&temp_root))
            .expect("discover games should succeed");

        assert_eq!(
            summary.discovered_games.len(),
            1,
            "expected one game, got: {:?}",
            summary
                .discovered_games
                .iter()
                .map(|g| g.active_save_dir.display().to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(summary.discovered_games[0].name, "Cyberpunk 2077");
        assert_eq!(summary.discovered_games[0].active_save_dir, game_dir);

        fs::remove_dir_all(temp_root).expect("cleanup temp dir");
    }

    #[test]
    fn save_slot_directory_name_detection() {
        assert!(super::is_save_slot_directory_name("AutoSave-1"));
        assert!(super::is_save_slot_directory_name("Quicksave"));
        assert!(!super::is_save_slot_directory_name("Cyberpunk 2077"));
        assert!(!super::is_save_slot_directory_name("MyGame"));
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("slotforge_{prefix}_{ts}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
