use serde::Deserialize;
use slotforge::api::{
    from_anyhow, AddGameResultDto, ApiResponse, BackupResultDto, IgnoreGameResultDto,
    IgnoredListDto, LibraryStateDto, RestoreResultDto, SnapshotResultDto, VerifyAllResultDto,
};
use slotforge::domain::conflict::ResolutionChoice;
use slotforge::services::discovery_service::DiscoveredSaveFile;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddGameArgs {
    name: String,
    active_save_dir: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupGameArgs {
    game_id: String,
    label: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreSnapshotArgs {
    snapshot_id: String,
    resolution_choice: Option<ResolutionChoice>,
    confirmed_destructive: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotIdArgs {
    snapshot_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameIdArgs {
    game_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAnnotationArgs {
    snapshot_id: String,
    label: Option<String>,
    note: Option<String>,
    label_color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSnapshotArgs {
    snapshot_id: String,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanDirectoryArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddIgnoredPathArgs {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IgnoredPathArgs {
    path: String,
}

#[tauri::command]
fn load_library() -> ApiResponse<LibraryStateDto> {
    from_anyhow(slotforge::api::load_library())
}

#[tauri::command]
fn scan_games() -> ApiResponse<LibraryStateDto> {
    from_anyhow(slotforge::api::scan_games())
}

#[tauri::command]
async fn scan_games_background() -> ApiResponse<LibraryStateDto> {
    match tauri::async_runtime::spawn_blocking(slotforge::api::scan_games).await {
        Ok(result) => from_anyhow(result),
        Err(err) => ApiResponse::failure("JOIN", err.to_string()),
    }
}

#[tauri::command]
fn add_game(args: AddGameArgs) -> ApiResponse<AddGameResultDto> {
    from_anyhow(slotforge::api::add_game(&args.name, &args.active_save_dir))
}

#[tauri::command]
fn backup_game(args: BackupGameArgs) -> ApiResponse<BackupResultDto> {
    from_anyhow(slotforge::api::backup_game(
        &args.game_id,
        args.label,
        args.note,
    ))
}

#[tauri::command]
fn restore_snapshot(args: RestoreSnapshotArgs) -> ApiResponse<RestoreResultDto> {
    from_anyhow(slotforge::api::restore_snapshot(
        &args.snapshot_id,
        args.resolution_choice,
        args.confirmed_destructive,
    ))
}

#[tauri::command]
fn rollback_swap() -> ApiResponse<LibraryStateDto> {
    from_anyhow(slotforge::api::rollback_swap())
}

#[tauri::command]
fn verify_snapshot(args: SnapshotIdArgs) -> ApiResponse<SnapshotResultDto> {
    from_anyhow(slotforge::api::verify_snapshot(&args.snapshot_id))
}

#[tauri::command]
fn verify_all_snapshots(args: GameIdArgs) -> ApiResponse<VerifyAllResultDto> {
    from_anyhow(slotforge::api::verify_all_snapshots(&args.game_id))
}

#[tauri::command]
fn update_annotation(args: UpdateAnnotationArgs) -> ApiResponse<SnapshotResultDto> {
    from_anyhow(slotforge::api::update_annotation(
        &args.snapshot_id,
        args.label,
        args.note,
        args.label_color,
    ))
}

#[tauri::command]
fn delete_snapshot(args: DeleteSnapshotArgs) -> ApiResponse<LibraryStateDto> {
    from_anyhow(slotforge::api::delete_snapshot(
        &args.snapshot_id,
        args.confirmed,
    ))
}

#[tauri::command]
fn scan_save_directory(args: ScanDirectoryArgs) -> ApiResponse<Vec<DiscoveredSaveFile>> {
    from_anyhow(slotforge::api::scan_save_directory(&args.path))
}

#[tauri::command]
fn destructive_restore_warning(args: SnapshotIdArgs) -> ApiResponse<String> {
    from_anyhow(slotforge::api::destructive_restore_warning(&args.snapshot_id))
}

#[tauri::command]
fn list_ignored_games() -> ApiResponse<IgnoredListDto> {
    from_anyhow(slotforge::api::list_ignored_games())
}

#[tauri::command]
fn add_ignored_path(args: AddIgnoredPathArgs) -> ApiResponse<IgnoredListDto> {
    from_anyhow(slotforge::api::add_ignored_path(&args.path, args.name))
}

#[tauri::command]
fn remove_ignored_path(args: IgnoredPathArgs) -> ApiResponse<IgnoredListDto> {
    from_anyhow(slotforge::api::remove_ignored_path(&args.path))
}

#[tauri::command]
fn ignore_game_from_library(args: GameIdArgs) -> ApiResponse<IgnoreGameResultDto> {
    from_anyhow(slotforge::api::ignore_game_from_library(&args.game_id))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(err) = run_app() {
        eprintln!("SlotForge desktop failed to start: {err}");
        std::process::exit(1);
    }
}

fn run_app() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_library,
            scan_games,
            scan_games_background,
            add_game,
            backup_game,
            restore_snapshot,
            rollback_swap,
            verify_snapshot,
            verify_all_snapshots,
            update_annotation,
            delete_snapshot,
            scan_save_directory,
            destructive_restore_warning,
            list_ignored_games,
            add_ignored_path,
            remove_ignored_path,
            ignore_game_from_library,
        ])
        .run(tauri::generate_context!())
}
