pub fn title() -> &'static str {
    "Settings"
}

use std::path::PathBuf;

use anyhow::Result;

use crate::services::config_service::{self, ConflictPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsScreenState {
    pub scan_paths: Vec<PathBuf>,
    pub vault_root: PathBuf,
    pub conflict_policy: ConflictPolicy,
    pub status_message: Option<String>,
    pub help_text: &'static str,
}

pub fn load_state() -> Result<SettingsScreenState> {
    Ok(SettingsScreenState {
        scan_paths: config_service::get_scan_paths()?,
        vault_root: config_service::get_vault_root()?,
        conflict_policy: config_service::get_conflict_policy()?,
        status_message: None,
        help_text:
            "Changes are saved instantly. Keep conflict policy on Prompt Always for maximum safety.",
    })
}

pub fn current_conflict_policy() -> Result<ConflictPolicy> {
    config_service::get_conflict_policy()
}

pub fn update_conflict_policy(policy: ConflictPolicy) -> Result<ConflictPolicy> {
    let config = config_service::set_conflict_policy(policy)?;
    Ok(config.conflict_policy)
}

pub fn add_scan_path(path: &str) -> Result<SettingsScreenState> {
    config_service::add_scan_path(path)?;
    let mut state = load_state()?;
    state.status_message = Some("Scan location added.".to_string());
    Ok(state)
}

pub fn remove_scan_path(path: &str) -> Result<SettingsScreenState> {
    config_service::remove_scan_path(path)?;
    let mut state = load_state()?;
    state.status_message = Some("Scan location removed.".to_string());
    Ok(state)
}

pub fn update_vault_root(path: &str) -> Result<SettingsScreenState> {
    config_service::set_vault_root(path)?;
    let mut state = load_state()?;
    state.status_message = Some("Vault location updated.".to_string());
    Ok(state)
}
