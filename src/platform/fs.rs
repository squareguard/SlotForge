use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

pub fn expand_user_and_env(raw: &str) -> PathBuf {
    let with_home = if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(stripped).to_string_lossy().into_owned()
        } else {
            raw.to_string()
        }
    } else {
        raw.to_string()
    };

    // Supports `%VAR%` (Windows-style) and `${VAR}` (POSIX-style).
    let expanded = expand_env_markers(&with_home);
    PathBuf::from(expanded)
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

pub fn resolve_path(raw: &str) -> PathBuf {
    normalize_path(&expand_user_and_env(raw))
}

pub fn ensure_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    Ok(())
}

/// Ensures `path` exists and is a directory (after expand/normalize).
pub fn validate_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("directory does not exist: {}", path.display());
    }
    if !path.is_dir() {
        anyhow::bail!("path is not a directory: {}", path.display());
    }
    Ok(())
}

/// Walks `root` up to `max_depth`, returning all paths; propagates walk errors.
pub fn walk_tree(root: &Path, max_depth: u32) -> Result<Vec<PathBuf>> {
    use walkdir::WalkDir;

    let mut paths = Vec::new();
    for entry in WalkDir::new(root)
        .max_depth(max_depth as usize)
        .follow_links(false)
    {
        let entry = entry.with_context(|| {
            format!("failed to read directory entry under {}", root.display())
        })?;
        paths.push(entry.into_path());
    }
    Ok(paths)
}

pub fn normalize_and_dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for path in paths {
        let normalized = normalize_path(&path);
        let key = normalized.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            deduped.push(normalized);
        }
    }

    deduped
}

fn expand_env_markers(input: &str) -> String {
    let mut output = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '%' {
            let start = i + 1;
            if let Some(end_offset) = chars[start..].iter().position(|c| *c == '%') {
                let end = start + end_offset;
                let key: String = chars[start..end].iter().collect();
                if let Ok(value) = env::var(&key) {
                    output.push_str(&value);
                    i = end + 1;
                    continue;
                }
            }
        }

        if i + 2 < chars.len() && chars[i] == '$' && chars[i + 1] == '{' {
            let start = i + 2;
            if let Some(end_offset) = chars[start..].iter().position(|c| *c == '}') {
                let end = start + end_offset;
                let key: String = chars[start..end].iter().collect();
                if let Ok(value) = env::var(&key) {
                    output.push_str(&value);
                    i = end + 1;
                    continue;
                }
            }
        }

        output.push(chars[i]);
        i += 1;
    }

    output
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{normalize_path, resolve_path};

    #[test]
    fn normalize_removes_dot_segments() {
        let input = PathBuf::from("games/./slotforge/../saves");
        let normalized = normalize_path(&input);
        assert_eq!(normalized, PathBuf::from("games/saves"));
    }

    #[cfg(windows)]
    #[test]
    fn resolve_windows_env_style_path() {
        // SAFETY: scoped test-only environment override.
        unsafe { std::env::set_var("SLOTFORGE_TEST_ROOT", r"C:\Games") };
        let resolved = resolve_path(r"%SLOTFORGE_TEST_ROOT%\CallOfDuty\..\Saves");
        assert!(resolved.to_string_lossy().contains(r"C:\Games"));
        assert!(resolved.to_string_lossy().ends_with(r"Saves"));
        // SAFETY: cleanup for scoped test-only override.
        unsafe { std::env::remove_var("SLOTFORGE_TEST_ROOT") };
    }

    #[cfg(not(windows))]
    #[test]
    fn resolve_unix_env_style_path() {
        // SAFETY: scoped test-only environment override.
        unsafe { std::env::set_var("SLOTFORGE_TEST_ROOT", "/tmp/games") };
        let resolved = resolve_path("${SLOTFORGE_TEST_ROOT}/mygame/../saves");
        assert_eq!(resolved, PathBuf::from("/tmp/games/saves"));
        // SAFETY: cleanup for scoped test-only override.
        unsafe { std::env::remove_var("SLOTFORGE_TEST_ROOT") };
    }
}
