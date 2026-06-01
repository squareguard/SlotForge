use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use fs2::available_space;

use crate::domain::conflict::{ConflictComparison, ResolutionChoice, SaveFreshness};
use crate::domain::game::GameRecord;
use crate::domain::save::{SaveOrigin, SaveRecord};
use crate::services::metrics_service::{self, MetricOperation};
use crate::services::{config_service, metadata_service};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapPreflightRequest {
    pub source_save: SaveRecord,
    pub destination_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapPreflightReport {
    pub source_exists: bool,
    pub source_is_file: bool,
    pub destination_exists: bool,
    pub destination_is_dir: bool,
    pub source_readable: bool,
    pub destination_writable: bool,
    pub required_bytes: u64,
    pub available_bytes: Option<u64>,
}

impl SwapPreflightReport {
    pub fn is_ready(&self) -> bool {
        self.source_exists
            && self.source_is_file
            && self.destination_exists
            && self.destination_is_dir
            && self.source_readable
            && self.destination_writable
            && self
                .available_bytes
                .map(|available| available >= self.required_bytes)
                .unwrap_or(false)
    }
}

pub fn preflight_check(request: &SwapPreflightRequest) -> Result<SwapPreflightReport> {
    let source = &request.source_save.absolute_path;
    let destination = &request.destination_dir;

    let source_exists = source.exists();
    let source_is_file = source_exists && source.is_file();
    let destination_exists = destination.exists();
    let destination_is_dir = destination_exists && destination.is_dir();

    let source_readable = source_is_file && can_read(source);
    let destination_writable = destination_is_dir && can_write_to_dir(destination);
    let required_bytes = request.source_save.metadata.byte_size;
    let available_bytes = if destination_is_dir {
        available_space(destination).ok()
    } else {
        None
    };

    Ok(SwapPreflightReport {
        source_exists,
        source_is_file,
        destination_exists,
        destination_is_dir,
        source_readable,
        destination_writable,
        required_bytes,
        available_bytes,
    })
}

pub fn ensure_preflight_ready(request: &SwapPreflightRequest) -> Result<SwapPreflightReport> {
    let report = preflight_check(request)?;
    if !report.is_ready() {
        anyhow::bail!(
            "swap preflight failed (src_exists={}, src_file={}, dst_exists={}, dst_dir={}, src_readable={}, dst_writable={}, required_bytes={}, available_bytes={:?})",
            report.source_exists,
            report.source_is_file,
            report.destination_exists,
            report.destination_is_dir,
            report.source_readable,
            report.destination_writable,
            report.required_bytes,
            report.available_bytes
        );
    }
    Ok(report)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapTransactionRequest {
    pub game: GameRecord,
    pub selected_vault_save: SaveRecord,
    pub resolution_choice: Option<ResolutionChoice>,
    pub confirmed_destructive_action: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapTransactionResult {
    pub staged_active_backup_path: Option<PathBuf>,
    pub active_destination_path: PathBuf,
    pub active_metadata: crate::domain::save::SaveMetadata,
}

pub fn execute_swap_transaction(request: &SwapTransactionRequest) -> Result<SwapTransactionResult> {
    let mut recovered_after_failure = false;
    let outcome = (|| {
        let active_destination_path = request
            .game
            .active_save_dir
            .join(&request.selected_vault_save.file_name);

        let preflight = SwapPreflightRequest {
            source_save: request.selected_vault_save.clone(),
            destination_dir: request.game.active_save_dir.clone(),
        };
        ensure_preflight_ready(&preflight)?;

        let active_existing = if active_destination_path.exists() {
            Some(read_active_save_record(&request.game, &active_destination_path)?)
        } else {
            None
        };

        if let Some(active_record) = &active_existing {
            if !request.confirmed_destructive_action {
                anyhow::bail!(
                    "swap blocked: confirmation required before replacing active save"
                );
            }
            let comparison = compare_for_swap(&request.selected_vault_save, active_record);
            let resolved_choice =
                resolve_conflict_choice(request.resolution_choice.clone(), &comparison)?;
            apply_resolution_choice(&resolved_choice, &comparison)?;
        }

        let staged_active_backup_path = if active_existing.is_some() {
            Some(stage_existing_active_save(&request.game, &active_destination_path)?)
        } else {
            None
        };

        let swap_result: Result<()> = (|| {
            fs::copy(
                &request.selected_vault_save.absolute_path,
                &active_destination_path,
            )
            .with_context(|| {
                format!(
                    "failed to copy selected save {} into active location {}",
                    request.selected_vault_save.absolute_path.display(),
                    active_destination_path.display()
                )
            })?;

            let active_metadata = metadata_service::collect_metadata(&active_destination_path)?;
            metadata_service::verify_metadata_pair(
                &request.selected_vault_save.metadata,
                &active_metadata,
            )
            .context("swap verification failed after file placement")?;
            Ok(())
        })();

        if let Err(swap_error) = swap_result {
            rollback_swap_failure(
                &active_destination_path,
                staged_active_backup_path.as_deref(),
            )?;
            recovered_after_failure = true;
            return Err(swap_error);
        }

        let active_metadata = metadata_service::collect_metadata(&active_destination_path)?;

        Ok(SwapTransactionResult {
            staged_active_backup_path,
            active_destination_path,
            active_metadata,
        })
    })();

    match &outcome {
        Ok(_) => {
            metrics_service::record_operation_best_effort(MetricOperation::Swap, true, false, false);
        }
        Err(err) => {
            metrics_service::record_operation_best_effort(
                MetricOperation::Swap,
                false,
                metrics_service::is_likely_user_error(&err.to_string()),
                recovered_after_failure,
            );
        }
    }
    outcome
}

pub fn destructive_swap_warning(selected: &SaveRecord, existing_active: Option<&SaveRecord>) -> String {
    if let Some(active) = existing_active {
        return format!(
            "Warning: swapping '{}' will replace active save '{}'. The active file is staged to vault first, but confirm before continuing.",
            selected.file_name, active.file_name
        );
    }
    format!(
        "Warning: swapping '{}' writes directly into the active save directory.",
        selected.file_name
    )
}

fn can_read(path: &Path) -> bool {
    fs::File::open(path).is_ok()
}

fn can_write_to_dir(path: &Path) -> bool {
    let probe = path.join(".slotforge_write_probe.tmp");
    let result = fs::write(&probe, b"slotforge-probe");
    if result.is_ok() {
        let _ = fs::remove_file(&probe);
        return true;
    }
    false
}

fn stage_existing_active_save(game: &GameRecord, active_path: &Path) -> Result<PathBuf> {
    let config = config_service::ensure_initialized()?;
    let game_vault_dir = config.vault_root.join(sanitize_segment(&game.name));
    fs::create_dir_all(&game_vault_dir).with_context(|| {
        format!(
            "failed to create vault staging directory {}",
            game_vault_dir.display()
        )
    })?;

    let file_name = active_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "save.dat".to_string());
    let staged_path = unique_destination(&game_vault_dir, &file_name);

    match fs::rename(active_path, &staged_path) {
        Ok(_) => {}
        Err(_) => {
            fs::copy(active_path, &staged_path)?;
            fs::remove_file(active_path)?;
        }
    }

    Ok(staged_path)
}

/// Undo a successful swap by restoring the staged active save (if any) and removing the restored file.
pub fn rollback_user_swap(active_destination_path: &Path, staged_backup_path: Option<&Path>) -> Result<()> {
    rollback_swap_failure(active_destination_path, staged_backup_path)
}

fn rollback_swap_failure(active_destination_path: &Path, staged_backup_path: Option<&Path>) -> Result<()> {
    if active_destination_path.exists() && active_destination_path.is_file() {
        fs::remove_file(active_destination_path).with_context(|| {
            format!(
                "rollback failed to remove partially swapped file {}",
                active_destination_path.display()
            )
        })?;
    }

    if let Some(staged_path) = staged_backup_path {
        if staged_path.exists() && staged_path.is_file() {
            fs::rename(staged_path, active_destination_path)
                .or_else(|_| {
                    fs::copy(staged_path, active_destination_path)?;
                    fs::remove_file(staged_path)
                })
                .with_context(|| {
                    format!(
                        "rollback failed to restore staged backup {} back to {}",
                        staged_path.display(),
                        active_destination_path.display()
                    )
                })?;
        }
    }

    Ok(())
}

fn unique_destination(vault_dir: &Path, file_name: &str) -> PathBuf {
    let initial = vault_dir.join(file_name);
    if !initial.exists() {
        return initial;
    }

    let ts = Utc::now().format("%Y%m%d%H%M%S");
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|v| v.to_str()).unwrap_or("save");
    let ext = path.extension().and_then(|v| v.to_str()).unwrap_or("");

    for idx in 1..=5000 {
        let candidate = if ext.is_empty() {
            vault_dir.join(format!("{stem}_{ts}_{idx}"))
        } else {
            vault_dir.join(format!("{stem}_{ts}_{idx}.{ext}"))
        };
        if !candidate.exists() {
            return candidate;
        }
    }

    vault_dir.join(format!("{stem}_{ts}_overflow"))
}

fn sanitize_segment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "UnknownGame".to_string()
    } else {
        trimmed.to_string()
    }
}

fn read_active_save_record(game: &GameRecord, active_path: &Path) -> Result<SaveRecord> {
    let file_name = active_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "save.dat".to_string());
    Ok(SaveRecord {
        id: format!("active:{}:{}", game.id, active_path.to_string_lossy().to_lowercase()),
        game_id: game.id.clone(),
        file_name,
        absolute_path: active_path.to_path_buf(),
        origin: SaveOrigin::ActiveDirectory,
        label: None,
        note: None,
        label_color: None,
        metadata: metadata_service::collect_metadata(active_path)?,
        archived_at: None,
    })
}

fn compare_for_swap(source: &SaveRecord, destination: &SaveRecord) -> ConflictComparison {
    let freshness = match (
        source.metadata.modified_at,
        destination.metadata.modified_at,
        source.metadata.sha256.as_deref(),
        destination.metadata.sha256.as_deref(),
    ) {
        (Some(_), Some(_), Some(sha_a), Some(sha_b)) if sha_a == sha_b => SaveFreshness::Equal,
        (Some(a), Some(b), _, _) if a > b => SaveFreshness::SourceNewer,
        (Some(a), Some(b), _, _) if a < b => SaveFreshness::DestinationNewer,
        (Some(_), Some(_), _, _) => SaveFreshness::Equal,
        _ => SaveFreshness::Unknown,
    };

    let reason = match freshness {
        SaveFreshness::Equal => "files appear equivalent by timestamp/hash".to_string(),
        SaveFreshness::SourceNewer => "selected vault save is newer than active save".to_string(),
        SaveFreshness::DestinationNewer => "active save is newer than selected vault save".to_string(),
        SaveFreshness::Unknown => "unable to determine freshness confidently".to_string(),
    };

    ConflictComparison {
        source_path: source.absolute_path.to_string_lossy().to_string(),
        destination_path: destination.absolute_path.to_string_lossy().to_string(),
        source_metadata: source.metadata.clone(),
        destination_metadata: destination.metadata.clone(),
        freshness,
        reason,
    }
}

fn apply_resolution_choice(choice: &ResolutionChoice, comparison: &ConflictComparison) -> Result<()> {
    match (comparison.freshness.clone(), choice) {
        (SaveFreshness::DestinationNewer, ResolutionChoice::KeepDestination) => {
            anyhow::bail!("swap cancelled by resolution choice: keep destination")
        }
        (SaveFreshness::Unknown, ResolutionChoice::CancelOperation) => {
            anyhow::bail!("swap cancelled due to unknown freshness")
        }
        (_, ResolutionChoice::CancelOperation) => anyhow::bail!("swap cancelled by user choice"),
        (_, ResolutionChoice::KeepSource) => Ok(()),
        (_, ResolutionChoice::KeepBothRename) => Ok(()),
        (_, ResolutionChoice::KeepDestination) => Ok(()),
    }
}

fn resolve_conflict_choice(
    explicit_choice: Option<ResolutionChoice>,
    comparison: &ConflictComparison,
) -> Result<ResolutionChoice> {
    if let Some(choice) = explicit_choice {
        return Ok(choice);
    }

    let policy = config_service::get_conflict_policy()?;
    let choice = match policy {
        config_service::ConflictPolicy::PromptAlways => ResolutionChoice::CancelOperation,
        config_service::ConflictPolicy::KeepBothByDefault => ResolutionChoice::KeepBothRename,
        config_service::ConflictPolicy::PreferNewerWithPromptOnEqual => match comparison.freshness {
            SaveFreshness::SourceNewer => ResolutionChoice::KeepSource,
            SaveFreshness::DestinationNewer => ResolutionChoice::KeepDestination,
            SaveFreshness::Equal | SaveFreshness::Unknown => ResolutionChoice::CancelOperation,
        },
    };
    Ok(choice)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::{Duration, Utc};

    use crate::domain::conflict::{ResolutionChoice, SaveFreshness};
    use crate::domain::save::{SaveMetadata, SaveOrigin, SaveRecord};
    use crate::services::config_service::{self, ConflictPolicy};

    use super::{
        apply_resolution_choice, compare_for_swap, destructive_swap_warning, resolve_conflict_choice,
    };

    #[test]
    fn destructive_warning_mentions_replace_when_active_exists() {
        let selected = build_save("selected", Utc::now(), "aaa");
        let existing = build_save("existing", Utc::now() - Duration::minutes(1), "bbb");
        let warning = destructive_swap_warning(&selected, Some(&existing));
        assert!(warning.contains("replace active save"));
    }

    #[test]
    fn compare_for_swap_detects_destination_newer() {
        let source = build_save("source", Utc::now() - Duration::minutes(5), "a1");
        let destination = build_save("dest", Utc::now(), "b1");
        let comparison = compare_for_swap(&source, &destination);
        assert_eq!(comparison.freshness, SaveFreshness::DestinationNewer);
    }

    #[test]
    fn apply_resolution_choice_cancels_when_requested() {
        let source = build_save("source", Utc::now(), "h1");
        let destination = build_save("dest", Utc::now() - Duration::minutes(1), "h2");
        let comparison = compare_for_swap(&source, &destination);
        let result = apply_resolution_choice(&ResolutionChoice::CancelOperation, &comparison);
        assert!(result.is_err());
    }

    #[test]
    fn policy_prefer_newer_prompts_on_equal() {
        let config_path = unique_temp_file("policy");
        // SAFETY: tests run in-process; this test scopes and cleans up env override.
        unsafe { std::env::set_var("SLOTFORGE_CONFIG_PATH", config_path.to_string_lossy().to_string()) };
        config_service::set_conflict_policy(ConflictPolicy::PreferNewerWithPromptOnEqual)
            .expect("set policy");

        let source = build_save("source", Utc::now(), "same");
        let destination = build_save("dest", Utc::now() - Duration::minutes(4), "same");
        let comparison = compare_for_swap(&source, &destination);
        let choice =
            resolve_conflict_choice(None, &comparison).expect("resolve by policy should succeed");
        assert_eq!(choice, ResolutionChoice::CancelOperation);

        let _ = fs::remove_file(&config_path);
        unsafe { std::env::remove_var("SLOTFORGE_CONFIG_PATH") };
    }

    fn build_save(id_suffix: &str, modified_at: chrono::DateTime<Utc>, sha: &str) -> SaveRecord {
        SaveRecord {
            id: format!("save-{id_suffix}"),
            game_id: "game-a".to_string(),
            file_name: format!("{id_suffix}.sav"),
            absolute_path: std::path::PathBuf::from(format!("{id_suffix}.sav")),
            origin: SaveOrigin::Vault,
            label: None,
            note: None,
            label_color: None,
            metadata: SaveMetadata {
                modified_at: Some(modified_at),
                created_at: Some(modified_at),
                byte_size: 512,
                sha256: Some(sha.to_string()),
            },
            archived_at: Some(modified_at),
        }
    }

    fn unique_temp_file(prefix: &str) -> std::path::PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("slotforge_{prefix}_{ts}.json"))
    }
}
