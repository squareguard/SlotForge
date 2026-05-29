use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::platform::fs::ensure_directory;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditAction {
    BackupSave,
    RestoreSave,
    SwapSave,
    DeleteSave,
    ScanPaths,
    ConfigChanged,
    AppStartup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub action: AuditAction,
    pub outcome: AuditOutcome,
    pub message: String,
    pub game_id: Option<String>,
    pub source_path: Option<PathBuf>,
    pub destination_path: Option<PathBuf>,
}

pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

pub fn record_event(event: &AuditEvent) -> Result<()> {
    let json = serde_json::to_string(event).context("failed to serialize audit event")?;
    match event.outcome {
        AuditOutcome::Success => info!("{json}"),
        AuditOutcome::Failure => warn!("{json}"),
    }
    append_to_audit_log(json)
}

fn append_to_audit_log(line: String) -> Result<()> {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open audit log at {}", path.display()))?;
    writeln!(file, "{line}")
        .with_context(|| format!("failed to append audit line to {}", path.display()))?;
    Ok(())
}

fn audit_log_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("audit.log");
    }
    PathBuf::from("slotforge-audit.log")
}
