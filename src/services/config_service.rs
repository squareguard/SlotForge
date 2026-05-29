use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::platform::fs::{normalize_and_dedup_paths, resolve_path};
use crate::platform::path_defaults::{default_scan_paths, default_vault_root};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictPolicy {
    PromptAlways,
    KeepBothByDefault,
    PreferNewerWithPromptOnEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafetyOptions {
    pub require_confirmation_for_delete: bool,
    pub verify_hash_after_swap: bool,
    pub create_rollback_snapshots: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub vault_root: PathBuf,
    pub scan_paths: Vec<PathBuf>,
    pub conflict_policy: ConflictPolicy,
    pub safety: SafetyOptions,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            vault_root: default_vault_root(),
            scan_paths: default_scan_paths(),
            conflict_policy: ConflictPolicy::PromptAlways,
            safety: SafetyOptions {
                require_confirmation_for_delete: true,
                verify_hash_after_swap: true,
                create_rollback_snapshots: true,
            },
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("SLOTFORGE_CONFIG_PATH") {
        let resolved = resolve_path(&override_path);
        if !resolved.as_os_str().is_empty() {
            return resolved;
        }
    }
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("config.json");
    }
    PathBuf::from("slotforge-config.json")
}

pub fn ensure_initialized() -> Result<AppConfig> {
    let path = config_path();
    if path.exists() {
        return load_from_path(&path);
    }

    let config = AppConfig::default();
    save_to_path(&path, &config)?;
    Ok(config)
}

pub fn load_from_path(path: &Path) -> Result<AppConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;
    let config = serde_json::from_str::<AppConfig>(&raw)
        .with_context(|| format!("failed to parse JSON config at {}", path.display()))?;
    Ok(config)
}

pub fn save_to_path(path: &Path, config: &AppConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create config directory structure at {}",
                parent.display()
            )
        })?;
    }

    let json = serde_json::to_string_pretty(config).context("failed to serialize app config")?;
    fs::write(path, json)
        .with_context(|| format!("failed to write config file at {}", path.display()))?;
    Ok(())
}

pub fn get_scan_paths() -> Result<Vec<PathBuf>> {
    let mut config = ensure_initialized()?;
    config.scan_paths = normalize_and_dedup_paths(config.scan_paths);
    save_to_path(&config_path(), &config)?;
    Ok(config.scan_paths)
}

pub fn add_scan_path(raw_path: &str) -> Result<AppConfig> {
    let mut config = ensure_initialized()?;
    let resolved = resolve_path(raw_path);
    config.scan_paths.push(resolved);
    config.scan_paths = normalize_and_dedup_paths(config.scan_paths);
    save_to_path(&config_path(), &config)?;
    Ok(config)
}

pub fn remove_scan_path(raw_path: &str) -> Result<AppConfig> {
    let mut config = ensure_initialized()?;
    let resolved = resolve_path(raw_path);
    let resolved_key = resolved.to_string_lossy().to_lowercase();
    config.scan_paths.retain(|existing| {
        existing.to_string_lossy().to_lowercase() != resolved_key
    });
    config.scan_paths = normalize_and_dedup_paths(config.scan_paths);
    save_to_path(&config_path(), &config)?;
    Ok(config)
}

pub fn get_conflict_policy() -> Result<ConflictPolicy> {
    let config = ensure_initialized()?;
    Ok(config.conflict_policy.clone())
}

pub fn set_conflict_policy(policy: ConflictPolicy) -> Result<AppConfig> {
    let mut config = ensure_initialized()?;
    config.conflict_policy = policy;
    save_to_path(&config_path(), &config)?;
    Ok(config)
}

pub fn get_vault_root() -> Result<PathBuf> {
    let config = ensure_initialized()?;
    Ok(config.vault_root)
}

pub fn set_vault_root(raw_path: &str) -> Result<AppConfig> {
    let mut config = ensure_initialized()?;
    let resolved = resolve_path(raw_path);
    if resolved.as_os_str().is_empty() {
        anyhow::bail!("vault root path cannot be empty");
    }
    config.vault_root = resolved;
    save_to_path(&config_path(), &config)?;
    Ok(config)
}
