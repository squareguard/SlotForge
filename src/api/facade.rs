use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use crate::api::dto::{
    self, AddGameResultDto, BackupResultDto, IntegrityStatusDto, LibraryStateDto, RestoreResultDto,
    SnapshotDto, SnapshotResultDto, VerifyAllResultDto,
};
use crate::api::swap_session::{self, LastSwapSession};
use crate::domain::conflict::ResolutionChoice;
use crate::domain::game::GameRecord;
use crate::domain::save::{SaveOrigin, SaveRecord};
use crate::services::config_service;
use crate::services::discovery_service::{self, DiscoveredSaveFile};
use crate::services::library_service;
use crate::services::metadata_service;
use crate::services::swap_service::{self, SwapTransactionRequest};
use crate::services::vault_service::{self, DeleteRequest, DeleteTarget};
use crate::ui::screens::library_screen::{self, LibraryFilters};

static VERIFIED_IDS: std::sync::Mutex<Option<HashMap<String, bool>>> = std::sync::Mutex::new(None);

fn verified_cache() -> Result<HashMap<String, bool>> {
    let mut guard = VERIFIED_IDS
        .lock()
        .map_err(|_| anyhow::anyhow!("integrity cache lock poisoned"))?;
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    Ok(guard.as_mut().unwrap().clone())
}

fn set_verified(save_id: &str, ok: bool) -> Result<()> {
    let mut guard = VERIFIED_IDS
        .lock()
        .map_err(|_| anyhow::anyhow!("integrity cache lock poisoned"))?;
    let cache = guard.get_or_insert_with(HashMap::new);
    cache.insert(save_id.to_string(), ok);
    Ok(())
}

fn integrity_for(save_id: &str) -> Result<IntegrityStatusDto> {
    let cache = verified_cache()?;
    Ok(dto::integrity_from_verified(cache.get(save_id).copied()))
}

pub fn load_library() -> Result<LibraryStateDto> {
    build_library_state()
}

pub fn scan_games() -> Result<LibraryStateDto> {
    let _ = discovery_service::discover_and_merge_library()?;
    build_library_state()
}

pub fn add_game(name: &str, active_save_dir: &str) -> Result<AddGameResultDto> {
    let name = name.trim();
    let active_save_dir = active_save_dir.trim();
    if name.is_empty() {
        anyhow::bail!("Game name is required.");
    }
    if active_save_dir.is_empty() {
        anyhow::bail!("Game path is required.");
    }

    library_service::add_manual_game(name, active_save_dir)?;
    let library = build_library_state()?;
    let game = library
        .games
        .iter()
        .find(|g| g.active_save_dir.eq_ignore_ascii_case(active_save_dir))
        .cloned()
        .context("added game not found in library")?;

    let discovered_count =
        discovery_service::scan_save_files_in_directory(Path::new(active_save_dir))?.len();

    Ok(AddGameResultDto {
        library,
        game,
        discovered_count,
    })
}

pub fn backup_game(game_id: &str, label: Option<String>, note: Option<String>) -> Result<BackupResultDto> {
    let game = find_game(game_id)?;
    let records = vault_service::backup_active_saves_for_game(&game)?;

    if records.is_empty() {
        anyhow::bail!("No save files found to back up in the active save directory.");
    }

    let primary = records
        .first()
        .context("backup returned no records")?
        .clone();

    if label.is_some() || note.is_some() {
        vault_service::annotate_save(&primary.id, label, note)?;
    }

    for record in &records {
        let _ = set_verified(&record.id, true);
    }

    let library = build_library_state()?;
    let snapshot = library
        .vault_by_game_id
        .get(game_id)
        .and_then(|list| list.iter().find(|s| s.id == primary.id))
        .cloned()
        .context("backup snapshot missing from library state")?;

    Ok(BackupResultDto { library, snapshot })
}

pub fn restore_snapshot(
    snapshot_id: &str,
    resolution_choice: Option<ResolutionChoice>,
    confirmed_destructive: bool,
) -> Result<RestoreResultDto> {
    let (game, snapshot) = find_game_and_snapshot(snapshot_id)?;
    if snapshot.origin != SaveOrigin::Vault {
        anyhow::bail!("Only vault snapshots can be restored to the active directory.");
    }

    let save_record = snapshot_record_from_dto(&game, &snapshot)?;
    let result = swap_service::execute_swap_transaction(&SwapTransactionRequest {
        game: game.clone(),
        selected_vault_save: save_record,
        resolution_choice,
        confirmed_destructive_action: confirmed_destructive,
    })?;

    let session = LastSwapSession {
        game_id: game.id.clone(),
        snapshot_id: snapshot.id.clone(),
        active_destination_path: result.active_destination_path.clone(),
        staged_active_backup_path: result.staged_active_backup_path.clone(),
        restored_at: Utc::now(),
    };
    swap_session::save_last_swap(&session)?;

    let library = build_library_state()?;
    let last_swap = swap_session::session_to_dto(&session);
    Ok(RestoreResultDto { library, last_swap })
}

pub fn rollback_swap() -> Result<LibraryStateDto> {
    let session = swap_session::load_last_swap()?.context("No swap to roll back.")?;
    swap_service::rollback_user_swap(
        swap_session::active_destination(&session),
        swap_session::staged_backup(&session),
    )?;
    swap_session::clear_last_swap()?;
    build_library_state()
}

pub fn verify_snapshot(snapshot_id: &str) -> Result<SnapshotResultDto> {
    let (_, snapshot) = find_game_and_snapshot(snapshot_id)?;
    let path = PathBuf::from(&snapshot.absolute_path);
    if !path.is_file() {
        anyhow::bail!("Save file not found on disk.");
    }

    let fresh = metadata_service::collect_metadata(&path)?;
    let ok = snapshot
        .metadata
        .sha256
        .as_deref()
        .zip(fresh.sha256.as_deref())
        .map(|(a, b)| a == b)
        .unwrap_or(true);
    set_verified(snapshot_id, ok)?;

    let library = build_library_state()?;
    let snapshot = library
        .vault_by_game_id
        .values()
        .flat_map(|list| list.iter())
        .find(|s| s.id == snapshot_id)
        .cloned()
        .context("snapshot missing after verify")?;

    Ok(SnapshotResultDto { library, snapshot })
}

pub fn verify_all_snapshots(game_id: &str) -> Result<VerifyAllResultDto> {
    let game = find_game(game_id)?;
    let saves = vault_service::list_vault_saves_for_game(&game)?;
    let mut verified_count = 0usize;
    for save in saves {
        if verify_snapshot(&save.id).is_ok() {
            verified_count += 1;
        }
    }
    let library = build_library_state()?;
    Ok(VerifyAllResultDto {
        library,
        verified_count,
    })
}

pub fn update_annotation(
    snapshot_id: &str,
    label: Option<String>,
    note: Option<String>,
) -> Result<SnapshotResultDto> {
    vault_service::annotate_save(snapshot_id, label, note)?;
    let library = build_library_state()?;
    let snapshot = library
        .vault_by_game_id
        .values()
        .flat_map(|list| list.iter())
        .find(|s| s.id == snapshot_id)
        .cloned()
        .context("snapshot missing after annotation")?;
    Ok(SnapshotResultDto { library, snapshot })
}

pub fn delete_snapshot(snapshot_id: &str, confirmed: bool) -> Result<LibraryStateDto> {
    let (_, snapshot) = find_game_and_snapshot(snapshot_id)?;
    if snapshot.origin != SaveOrigin::Vault {
        anyhow::bail!("Only vault snapshots can be deleted from the vault.");
    }

    vault_service::delete_save(DeleteRequest {
        save_id: snapshot.id.clone(),
        absolute_path: PathBuf::from(&snapshot.absolute_path),
        target: DeleteTarget::Vault,
        confirmed,
        confirmation_phrase: if confirmed {
            Some("DELETE".to_string())
        } else {
            None
        },
    })?;

    build_library_state()
}

pub fn scan_save_directory(path: &str) -> Result<Vec<DiscoveredSaveFile>> {
    discovery_service::scan_save_files_in_directory(Path::new(path.trim()))
}

pub fn destructive_restore_warning(snapshot_id: &str) -> Result<String> {
    let (game, snapshot) = find_game_and_snapshot(snapshot_id)?;
    let vault_record = snapshot_record_from_dto(&game, &snapshot)?;
    let active_path = game.active_save_dir.join(&snapshot.file_name);
    let active_existing = if active_path.is_file() {
        Some(read_active_record(&game, &active_path)?)
    } else {
        None
    };
    Ok(swap_service::destructive_swap_warning(
        &vault_record,
        active_existing.as_ref(),
    ))
}

fn build_library_state() -> Result<LibraryStateDto> {
    crate::services::audit_service::init_logging();
    let _ = config_service::ensure_initialized()?;

    let filters = LibraryFilters::default();
    let screen = library_screen::load_state(filters)?;
    let games = screen.items;

    let mut vault_by_game_id = HashMap::new();
    for game in &games {
        let snapshots = build_snapshots_for_game(game)?;
        vault_by_game_id.insert(game.id.clone(), snapshots);
    }

    let last_swap = swap_session::load_last_swap()?.map(|s| swap_session::session_to_dto(&s));

    let mut game_dtos = Vec::new();
    for game in &games {
        let vault = vault_by_game_id.get(&game.id).cloned().unwrap_or_default();
        let last_backed_up = vault
            .iter()
            .filter(|s| s.origin == SaveOrigin::Vault)
            .filter_map(|s| s.archived_at.as_deref())
            .max()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());

        let (has_conflict, conflict_files) = detect_conflicts(game, &vault)?;
        game_dtos.push(dto::game_to_dto(
            game,
            last_backed_up,
            has_conflict,
            conflict_files,
        ));
    }

    Ok(LibraryStateDto {
        games: game_dtos,
        vault_by_game_id,
        last_swap,
    })
}

fn build_snapshots_for_game(game: &GameRecord) -> Result<Vec<SnapshotDto>> {
    let mut records = list_active_saves(game)?;
    let mut vault = vault_service::list_vault_saves_for_game(game)?;
    records.append(&mut vault);

    let mut snapshots = Vec::new();
    for record in records {
        let integrity = integrity_for(&record.id)?;
        let files = list_relative_files_for_save(&record)?;
        snapshots.push(dto::snapshot_to_dto(&record, integrity, files));
    }

    snapshots.sort_by(|a, b| {
        b.metadata
            .modified_at
            .cmp(&a.metadata.modified_at)
    });
    Ok(snapshots)
}

fn list_active_saves(game: &GameRecord) -> Result<Vec<SaveRecord>> {
    let dir = &game.active_save_dir;
    if !dir.exists() || !dir.is_dir() {
        return Ok(Vec::new());
    }

    let discovered = discovery_service::scan_save_files_in_directory(dir)?;
    let mut records = Vec::new();
    for file in discovered {
        let path = PathBuf::from(&file.absolute_path);
        let metadata = metadata_service::collect_metadata(&path)?;
        records.push(SaveRecord {
            id: format!("active:{}:{}", game.id, file.absolute_path.to_lowercase()),
            game_id: game.id.clone(),
            file_name: file.name,
            absolute_path: path,
            origin: SaveOrigin::ActiveDirectory,
            label: None,
            note: None,
            metadata,
            archived_at: None,
        });
    }

    vault_service::apply_annotations(records)
}

fn list_relative_files_for_save(record: &SaveRecord) -> Result<Vec<String>> {
    let path = &record.absolute_path;
    if path.is_file() {
        return Ok(vec![record.file_name.clone()]);
    }
    Ok(Vec::new())
}

fn detect_conflicts(
    game: &GameRecord,
    vault_snapshots: &[SnapshotDto],
) -> Result<(bool, Vec<dto::ConflictFileDto>)> {
    let mut conflict_files = Vec::new();
    for snapshot in vault_snapshots {
        if snapshot.origin != SaveOrigin::Vault {
            continue;
        }
        let active_path = game.active_save_dir.join(&snapshot.file_name);
        if !active_path.is_file() {
            continue;
        }
        let vault_record = snapshot_record_from_dto(game, snapshot)?;
        let active_record = read_active_record(game, &active_path)?;
        let comparison = vault_service::compare_saves(&vault_record, &active_record);
        if comparison.freshness != crate::domain::conflict::SaveFreshness::Equal {
            conflict_files.push(dto::conflict_file_from_comparison(
                &snapshot.file_name,
                &comparison,
            ));
        }
    }
    Ok((!conflict_files.is_empty(), conflict_files))
}

fn find_game(game_id: &str) -> Result<GameRecord> {
    let filters = LibraryFilters::default();
    let screen = library_screen::load_state(filters)?;
    screen
        .items
        .into_iter()
        .find(|g| g.id == game_id)
        .with_context(|| format!("game '{game_id}' not found"))
}

fn find_game_and_snapshot(snapshot_id: &str) -> Result<(GameRecord, SnapshotDto)> {
    let library = build_library_state()?;
    for (game_id, snapshots) in &library.vault_by_game_id {
        if let Some(snapshot) = snapshots.iter().find(|s| s.id == snapshot_id) {
            let game = library
                .games
                .iter()
                .find(|g| &g.id == game_id)
                .context("game missing for snapshot")?;
            let game_record = find_game(&game.id)?;
            return Ok((game_record, snapshot.clone()));
        }
    }
    anyhow::bail!("snapshot '{snapshot_id}' not found")
}

fn snapshot_record_from_dto(game: &GameRecord, snapshot: &SnapshotDto) -> Result<SaveRecord> {
    Ok(SaveRecord {
        id: snapshot.id.clone(),
        game_id: game.id.clone(),
        file_name: snapshot.file_name.clone(),
        absolute_path: PathBuf::from(&snapshot.absolute_path),
        origin: snapshot.origin.clone(),
        label: snapshot.label.clone(),
        note: snapshot.note.clone(),
        metadata: crate::domain::save::SaveMetadata {
            modified_at: snapshot
                .metadata
                .modified_at
                .as_deref()
                .and_then(|s| s.parse().ok()),
            created_at: snapshot
                .metadata
                .created_at
                .as_deref()
                .and_then(|s| s.parse().ok()),
            byte_size: snapshot.metadata.byte_size,
            sha256: snapshot.metadata.sha256.clone(),
        },
        archived_at: snapshot
            .archived_at
            .as_deref()
            .and_then(|s| s.parse().ok()),
    })
}

fn read_active_record(game: &GameRecord, active_path: &Path) -> Result<SaveRecord> {
    let file_name = active_path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| "save.dat".to_string());
    Ok(SaveRecord {
        id: format!("active:{}:{}", game.id, active_path.to_string_lossy().to_lowercase()),
        game_id: game.id.clone(),
        file_name,
        absolute_path: active_path.to_path_buf(),
        origin: SaveOrigin::ActiveDirectory,
        label: None,
        note: None,
        metadata: metadata_service::collect_metadata(active_path)?,
        archived_at: None,
    })
}
