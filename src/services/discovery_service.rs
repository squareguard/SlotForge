use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use walkdir::WalkDir;

use crate::domain::game::{GameRecord, GameSource};
use crate::platform::fs::normalize_and_dedup_paths;
use crate::platform::path_defaults::default_scan_paths;
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

        for game_dir in collect_candidate_game_dirs(root) {
            if let Some(record) = game_record_from_dir(&game_dir) {
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

fn collect_candidate_game_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for entry in WalkDir::new(root)
        .max_depth(4)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if contains_save_files(path) {
            dirs.push(path.to_path_buf());
        }
    }

    normalize_and_dedup_paths(dirs)
}

fn contains_save_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|value| value.to_str()) {
            let ext = ext.to_ascii_lowercase();
            if matches!(
                ext.as_str(),
                "sav" | "save" | "dat" | "bak" | "profile" | "json"
            ) {
                return true;
            }
        }
    }

    false
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

        fs::remove_dir_all(temp_root).expect("cleanup temp dir");
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("slotforge_{prefix}_{ts}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
