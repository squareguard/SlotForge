use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use chrono::Utc;

use crate::app::AppShellState;
use crate::domain::save::{SaveMetadata, SaveOrigin, SaveRecord};
use crate::services::discovery_service;
use crate::services::metrics_service::{self, MvpSuccessCriteria};
use crate::services::swap_service::{self, SwapPreflightRequest};
use crate::ui::navigation::AppSection;
use crate::ui::screens::{
    about_screen, library_screen, settings_screen, vault_screen,
};
use crate::ui::screens::library_screen::{LibraryFilters, SortMode};

pub fn run_startup_report(shell: &mut AppShellState) -> Result<()> {
    let about = about_screen::about_info();
    println!("{} v{}", about.app_name, about.version);
    println!("{}", about.purpose);
    println!("Build: {} ({})", about.build_target, about.build_profile);

    shell.navigate_to(AppSection::Settings);
    let settings = settings_screen::load_state()?;
    println!();
    println!("[{}]", settings_screen::title());
    println!("  Vault: {}", settings.vault_root.display());
    println!("  Scan paths: {}", settings.scan_paths.len());
    println!("  Conflict policy: {:?}", settings.conflict_policy);

    shell.navigate_to(AppSection::Library);
    let default_discovery = discovery_service::discover_from_default_locations()?;
    let library = library_screen::load_state(LibraryFilters::default())?;
    for sort_mode in [
        SortMode::NameAsc,
        SortMode::NameDesc,
        SortMode::UpdatedNewestFirst,
    ] {
        let _ = library_screen::load_state(LibraryFilters {
            sort_mode,
            ..LibraryFilters::default()
        })?;
    }
    println!();
    println!("[{}]", "Library");
    println!(
        "  Games in library: {} ({} from default locations)",
        library.items.len(),
        default_discovery.discovered_games.len()
    );
    for game in library.items.iter().take(5) {
        println!("  - {} ({})", game.name, game.active_save_dir.display());
    }
    if library.items.len() > 5 {
        println!("  ... and {} more", library.items.len() - 5);
    }

    shell.navigate_to(AppSection::Vault);
    if let Some(game) = library.items.first() {
        let vault = vault_screen::load_state(game)?;
        println!();
        println!("[{}]", vault_screen::title());
        println!("  Game: {}", vault.game_name);
        println!("  Vaulted saves: {}", vault.saves.len());
        if vault.saves.len() >= 2 {
            let comparison = vault_screen::compare_saves(
                game,
                &vault.saves[0].id,
                &vault.saves[1].id,
            )?;
            println!("  Compare sample: {:?} — {}", comparison.freshness, comparison.reason);
        }
        log_swap_readiness(game, vault.saves.first())?;
    } else {
        println!();
        println!("[{}]", vault_screen::title());
        println!("  No games discovered yet; vault browse skipped.");
        log_swap_readiness_placeholder()?;
    }

    shell.navigate_to(AppSection::About);
    let metrics = metrics_service::read_snapshot()?;
    let evaluation =
        metrics_service::evaluate_mvp_criteria(&metrics, &MvpSuccessCriteria::default());
    println!();
    println!("[{}]", about_screen::title());
    println!(
        "  Operation success rate: {:.1}% ({} attempts)",
        metrics.operation_success_rate * 100.0,
        metrics.total_attempts
    );
    println!(
        "  Swap failure rate: {:.1}% | Restore failure rate: {:.1}%",
        metrics.swap_failure_rate * 100.0,
        metrics.restore_failure_rate * 100.0
    );
    println!(
        "  User-error recoverability: {:.1}%",
        metrics.user_error_recoverability_rate * 100.0
    );
    println!(
        "  MVP criteria: {}",
        if evaluation.meets_criteria {
            "met"
        } else {
            "not met"
        }
    );
    for failure in &evaluation.failures {
        println!("    - {failure}");
    }

    println!();
    println!(
        "Active section: {} | Set SLOTFORGE_SELF_TEST=1 to run non-destructive API checks.",
        shell.active_section_label()
    );
    Ok(())
}

fn log_swap_readiness(
    game: &crate::domain::game::GameRecord,
    vault_save: Option<&SaveRecord>,
) -> Result<()> {
    if let Some(save) = vault_save {
        let warning = swap_service::destructive_swap_warning(save, None);
        println!("  Swap: {}", warning);
        let preflight = SwapPreflightRequest {
            source_save: save.clone(),
            destination_dir: game.active_save_dir.clone(),
        };
        let report = swap_service::preflight_check(&preflight)?;
        println!(
            "  Swap preflight: ready={} (needs {} bytes, available {:?})",
            report.is_ready(),
            report.required_bytes,
            report.available_bytes
        );
    } else {
        log_swap_readiness_placeholder()?;
    }
    Ok(())
}

fn log_swap_readiness_placeholder() -> Result<()> {
    let placeholder = SaveRecord {
        id: "placeholder".to_string(),
        game_id: "placeholder".to_string(),
        file_name: "slot.sav".to_string(),
        absolute_path: PathBuf::from("slot.sav"),
        origin: SaveOrigin::Vault,
        label: None,
        note: None,
        label_color: None,
        metadata: SaveMetadata {
            modified_at: Some(Utc::now()),
            created_at: Some(Utc::now()),
            byte_size: 0,
            sha256: None,
        },
        archived_at: None,
    };
    println!(
        "  Swap: {}",
        swap_service::destructive_swap_warning(&placeholder, None)
    );
    Ok(())
}

/// Exercises screen actions and services in an isolated temp directory.
pub fn run_self_test() -> Result<()> {
    let temp_root = unique_temp_dir("self-test");
    let game_dir = temp_root.join("DemoGame");
    fs::create_dir_all(&game_dir)?;
    fs::write(game_dir.join("demo.sav"), b"slotforge-self-test")?;

    let config_path = temp_root.join("config.json");
    let vault_root = temp_root.join("vault");
    fs::create_dir_all(&vault_root)?;
    // SAFETY: self-test only (SLOTFORGE_SELF_TEST=1); redirects config to an isolated temp file.
    unsafe {
        std::env::set_var("SLOTFORGE_CONFIG_PATH", config_path.to_string_lossy().to_string());
    }

    let config = crate::services::config_service::ensure_initialized()?;
    crate::services::config_service::set_vault_root(vault_root.to_string_lossy().as_ref())?;
    crate::services::config_service::add_scan_path(game_dir.to_string_lossy().as_ref())?;
    let _ = config;

    let filters = LibraryFilters::default();
    let added =
        library_screen::add_manual_game_action("Self Test Game", game_dir.to_string_lossy().as_ref(), filters.clone())?;
    let game = added
        .items
        .iter()
        .find(|g| g.name == "Self Test Game")
        .context("self-test game not found after add")?
        .clone();

    library_screen::edit_manual_game_action(
        &game.id,
        "Self Test Game (edited)",
        game_dir.to_string_lossy().as_ref(),
        filters,
    )?;

    let backed_up = crate::services::vault_service::backup_active_saves_for_game(&game)?;
    assert!(!backed_up.is_empty(), "expected at least one backup");

    if let Some(save) = backed_up.first() {
        vault_screen::annotate_save_action(
            &game,
            &save.id,
            Some("Self-test slot".to_string()),
            Some("Created by SLOTFORGE_SELF_TEST".to_string()),
        )?;
        let _ = vault_screen::inspect_save(&game, &save.id)?;
    }

    let _ = settings_screen::update_conflict_policy(
        crate::services::config_service::ConflictPolicy::PromptAlways,
    )?;
    let _ = settings_screen::remove_scan_path(game_dir.to_string_lossy().as_ref())?;
    let _ = settings_screen::add_scan_path(game_dir.to_string_lossy().as_ref())?;

    crate::services::library_service::remove_manual_game(&game.id)?;

    if let Err(err) = fs::remove_dir_all(&temp_root) {
        eprintln!("self-test: failed to remove temp dir {}: {err}", temp_root.display());
    }
    unsafe {
        std::env::remove_var("SLOTFORGE_CONFIG_PATH");
    }
    Ok(())
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("slotforge_{prefix}_{ts}"))
}
