use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::conflict::{ConflictComparison, SaveFreshness};
use crate::domain::game::GameRecord;
use crate::domain::save::{SaveOrigin, SaveRecord};
use crate::platform::fs::{ensure_directory, walk_tree};
use crate::services::config_service;
use crate::services::metadata_service;
use crate::services::metrics_service::{self, MetricOperation};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AnnotationRegistry {
    entries: Vec<SaveAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SaveAnnotation {
    save_id: String,
    label: Option<String>,
    note: Option<String>,
    #[serde(default)]
    label_color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteTarget {
    ActiveDirectory,
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteRequest {
    pub save_id: String,
    pub absolute_path: PathBuf,
    pub target: DeleteTarget,
    pub confirmed: bool,
    pub confirmation_phrase: Option<String>,
}

pub fn backup_active_saves_for_game(game: &GameRecord) -> Result<Vec<SaveRecord>> {
    let outcome = (|| {
        let config = config_service::ensure_initialized()?;
        let game_vault_dir = config.vault_root.join(sanitize_segment(&game.name));
        ensure_directory(&game_vault_dir)?;

        let mut records = Vec::new();
        for source in collect_backup_candidates(&game.active_save_dir)? {
            let file_name = source
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "save.dat".to_string());
            let destination = unique_destination(&game_vault_dir, &file_name);
            fs::copy(&source, &destination).with_context(|| {
                format!(
                    "failed to backup save from {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            metadata_service::verify_copy_integrity(&source, &destination).with_context(|| {
                format!(
                    "backup integrity verification failed for {} -> {}",
                    source.display(),
                    destination.display()
                )
            })?;

            records.push(SaveRecord {
                id: format!(
                    "vault:{}:{}",
                    game.id,
                    destination.to_string_lossy().to_lowercase()
                ),
                game_id: game.id.clone(),
                file_name: destination
                    .file_name()
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or(file_name),
                absolute_path: destination.clone(),
                origin: SaveOrigin::Vault,
                label: None,
                note: None,
                label_color: None,
                metadata: metadata_service::collect_metadata(&destination)?,
                archived_at: Some(Utc::now()),
            });
        }

        apply_annotations(records)
    })();

    match &outcome {
        Ok(_) => {
            metrics_service::record_operation_best_effort(
                MetricOperation::Backup,
                true,
                false,
                false,
            );
        }
        Err(err) => {
            metrics_service::record_operation_best_effort(
                MetricOperation::Backup,
                false,
                metrics_service::is_likely_user_error(&err.to_string()),
                false,
            );
        }
    }
    outcome
}

pub fn list_vault_saves_for_game(game: &GameRecord) -> Result<Vec<SaveRecord>> {
    let config = config_service::ensure_initialized()?;
    let game_vault_dir = config.vault_root.join(sanitize_segment(&game.name));
    if !game_vault_dir.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for path in walk_tree(&game_vault_dir, 2)? {
        if !path.is_file() {
            continue;
        }

        let file_name = path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| "save.dat".to_string());
        let metadata = metadata_service::collect_metadata(&path)?;

        records.push(SaveRecord {
            id: format!(
                "vault:{}:{}",
                game.id,
                path.to_string_lossy().to_lowercase()
            ),
            game_id: game.id.clone(),
            file_name,
            absolute_path: path.to_path_buf(),
            origin: SaveOrigin::Vault,
            label: None,
            note: None,
            label_color: None,
            metadata,
            archived_at: None,
        });
    }

    records.sort_by_key(|b| std::cmp::Reverse(b.metadata.modified_at));
    apply_annotations(records)
}

pub fn delete_save(request: DeleteRequest) -> Result<()> {
    let outcome = (|| {
        let config = config_service::ensure_initialized()?;
        if config.safety.require_confirmation_for_delete && !request.confirmed {
            anyhow::bail!("delete confirmation required by safety settings");
        }
        if config.safety.require_confirmation_for_delete {
            let phrase = request.confirmation_phrase.clone().unwrap_or_default();
            if phrase.trim() != "DELETE" {
                anyhow::bail!("delete blocked: confirmation phrase must be exactly DELETE");
            }
        }

        if !request.absolute_path.exists() {
            anyhow::bail!(
                "cannot delete missing save at {}",
                request.absolute_path.display()
            );
        }
        if !request.absolute_path.is_file() {
            anyhow::bail!(
                "delete target is not a file: {}",
                request.absolute_path.display()
            );
        }

        if matches!(request.target, DeleteTarget::Vault) {
            let normalized_vault = config.vault_root.to_string_lossy().to_lowercase();
            let normalized_target = request.absolute_path.to_string_lossy().to_lowercase();
            if !normalized_target.starts_with(&normalized_vault) {
                anyhow::bail!(
                    "refusing to delete: target is outside configured vault root ({})",
                    config.vault_root.display()
                );
            }
        }

        fs::remove_file(&request.absolute_path).with_context(|| {
            format!(
                "failed to delete save at {}",
                request.absolute_path.display()
            )
        })?;

        clear_annotation(&request.save_id)
    })();

    match &outcome {
        Ok(_) => {
            metrics_service::record_operation_best_effort(
                MetricOperation::Delete,
                true,
                false,
                false,
            );
        }
        Err(err) => {
            metrics_service::record_operation_best_effort(
                MetricOperation::Delete,
                false,
                metrics_service::is_likely_user_error(&err.to_string()),
                false,
            );
        }
    }
    outcome
}

pub fn annotate_save(
    save_id: &str,
    label: Option<String>,
    note: Option<String>,
    label_color: Option<String>,
) -> Result<()> {
    let mut registry = load_annotation_registry()?;

    let current = registry
        .entries
        .iter()
        .find(|entry| entry.save_id == save_id)
        .cloned();

    let final_label = match label {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => current.as_ref().and_then(|entry| entry.label.clone()),
    };

    let final_note = match note {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => current.as_ref().and_then(|entry| entry.note.clone()),
    };

    let final_color = match label_color {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => current.as_ref().and_then(|entry| entry.label_color.clone()),
    };

    registry.entries.retain(|entry| entry.save_id != save_id);
    if final_label.is_some() || final_note.is_some() || final_color.is_some() {
        registry.entries.push(SaveAnnotation {
            save_id: save_id.to_string(),
            label: final_label,
            note: final_note,
            label_color: final_color,
        });
    }

    save_annotation_registry(&registry)
}

pub fn set_label_color(save_id: &str, label_color: &str) -> Result<()> {
    let trimmed = label_color.trim();
    if trimmed.is_empty() {
        anyhow::bail!("label color cannot be empty");
    }
    let color = trimmed.to_string();

    let mut registry = load_annotation_registry()?;
    if let Some(entry) = registry.entries.iter_mut().find(|e| e.save_id == save_id) {
        entry.label_color = Some(color);
    } else {
        registry.entries.push(SaveAnnotation {
            save_id: save_id.to_string(),
            label: None,
            note: None,
            label_color: Some(color),
        });
    }
    save_annotation_registry(&registry)
}

pub fn apply_annotations(mut records: Vec<SaveRecord>) -> Result<Vec<SaveRecord>> {
    let registry = load_annotation_registry()?;
    for record in &mut records {
        if let Some(annotation) = registry.entries.iter().find(|a| a.save_id == record.id) {
            record.label = annotation.label.clone();
            record.note = annotation.note.clone();
            record.label_color = annotation.label_color.clone();
        }
    }
    Ok(records)
}

pub fn compare_saves(source: &SaveRecord, destination: &SaveRecord) -> ConflictComparison {
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
        SaveFreshness::Equal => {
            "timestamps and/or hashes indicate files are equivalent".to_string()
        }
        SaveFreshness::SourceNewer => "source file has newer modification timestamp".to_string(),
        SaveFreshness::DestinationNewer => {
            "destination file has newer modification timestamp".to_string()
        }
        SaveFreshness::Unknown => "insufficient metadata to determine freshness".to_string(),
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

fn collect_backup_candidates(active_dir: &Path) -> Result<Vec<PathBuf>> {
    if !active_dir.exists() || !active_dir.is_dir() {
        anyhow::bail!(
            "active save directory does not exist: {}",
            active_dir.display()
        );
    }

    let mut files = Vec::new();
    for path in walk_tree(active_dir, 2)? {
        if path.is_file() {
            files.push(path);
        }
    }
    Ok(files)
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

fn load_annotation_registry() -> Result<AnnotationRegistry> {
    let path = annotation_registry_path();
    if !path.exists() {
        return Ok(AnnotationRegistry::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read annotation registry at {}", path.display()))?;
    let registry = serde_json::from_str::<AnnotationRegistry>(&raw).with_context(|| {
        format!(
            "failed to parse annotation registry JSON at {}",
            path.display()
        )
    })?;
    Ok(registry)
}

fn clear_annotation(save_id: &str) -> Result<()> {
    let mut registry = load_annotation_registry()?;
    registry.entries.retain(|entry| entry.save_id != save_id);
    save_annotation_registry(&registry)
}

fn save_annotation_registry(registry: &AnnotationRegistry) -> Result<()> {
    let path = annotation_registry_path();
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let json = serde_json::to_string_pretty(registry)
        .context("failed to serialize annotation registry")?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write annotation registry at {}", path.display()))?;
    Ok(())
}

fn annotation_registry_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("slotforge").join("save-annotations.json");
    }
    PathBuf::from("slotforge-save-annotations.json")
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::domain::save::{SaveMetadata, SaveOrigin, SaveRecord};

    use super::compare_saves;

    #[test]
    fn compare_saves_reports_source_newer() {
        let newer = build_save("src", Utc::now(), "abc");
        let older = build_save("dst", Utc::now() - Duration::minutes(10), "def");

        let comparison = compare_saves(&newer, &older);
        assert_eq!(
            comparison.freshness,
            crate::domain::conflict::SaveFreshness::SourceNewer
        );
    }

    #[test]
    fn compare_saves_reports_equal_on_matching_hash() {
        let left = build_save("left", Utc::now(), "same");
        let right = build_save("right", Utc::now() - Duration::hours(1), "same");

        let comparison = compare_saves(&left, &right);
        assert_eq!(
            comparison.freshness,
            crate::domain::conflict::SaveFreshness::Equal
        );
    }

    fn build_save(id_suffix: &str, modified_at: chrono::DateTime<Utc>, sha: &str) -> SaveRecord {
        SaveRecord {
            id: format!("save-{id_suffix}"),
            game_id: "game-a".to_string(),
            file_name: format!("{id_suffix}.sav"),
            absolute_path: std::path::PathBuf::from(format!("/tmp/{id_suffix}.sav")),
            origin: SaveOrigin::Vault,
            label: None,
            note: None,
            label_color: None,
            metadata: SaveMetadata {
                modified_at: Some(modified_at),
                created_at: Some(modified_at),
                byte_size: 1024,
                sha256: Some(sha.to_string()),
            },
            archived_at: Some(modified_at),
        }
    }
}
