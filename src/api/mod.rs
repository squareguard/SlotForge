pub mod dto;
pub mod error;
pub mod facade;
pub mod swap_session;

pub use error::{from_anyhow, ApiResponse};
pub use facade::{
    add_game, backup_game, delete_snapshot, destructive_restore_warning, load_library,
    rollback_swap, restore_snapshot, scan_games, scan_save_directory, update_annotation,
    verify_all_snapshots, verify_snapshot,
};
pub use dto::{
    AddGameResultDto, BackupResultDto, LibraryStateDto, RestoreResultDto, SnapshotResultDto,
    VerifyAllResultDto,
};
