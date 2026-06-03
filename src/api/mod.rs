//! Stable API surface for the Tauri desktop shell and integration tests.

pub mod dto;
pub mod error;
pub mod facade;
pub mod library_cache;
pub mod swap_session;

pub use dto::{
    AddGameResultDto, BackupResultDto, IgnoreGameResultDto, IgnoredEntryDto, IgnoredListDto,
    LibraryStateDto, RestoreResultDto, SnapshotResultDto, VerifyAllResultDto,
};
pub use error::{from_anyhow, ApiResponse};
pub use facade::{
    add_game, add_ignored_path, backup_game, delete_snapshot, destructive_restore_warning,
    ignore_game_from_library, list_ignored_games, load_library, remove_ignored_path,
    restore_snapshot, rollback_swap, save_library_cache, scan_games, scan_save_directory,
    update_annotation, verify_all_snapshots, verify_snapshot,
};
