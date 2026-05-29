use std::path::PathBuf;

use crate::platform::fs::normalize_and_dedup_paths;

pub fn default_vault_root() -> PathBuf {
    if let Some(documents_dir) = dirs::document_dir() {
        return documents_dir.join("SlotForge").join("Vault");
    }
    PathBuf::from("SlotForge").join("Vault")
}

pub fn default_scan_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join("Saved Games"));
        paths.push(home_dir.join("Documents").join("My Games"));
        paths.push(home_dir.join("Documents").join("Saved Games"));
    }

    if cfg!(target_os = "linux") {
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".local").join("share"));
            paths.push(home_dir.join(".config"));
        }
    }

    if cfg!(target_os = "macos") {
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join("Library").join("Application Support"));
        }
    }

    normalize_and_dedup_paths(paths)
}
