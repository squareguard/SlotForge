use anyhow::Result;

use crate::domain::game::{GameRecord, GameSource};
use crate::services::{discovery_service, library_service};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    NameAsc,
    NameDesc,
    UpdatedNewestFirst,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFilters {
    pub query: Option<String>,
    pub source: Option<GameSource>,
    pub sort_mode: SortMode,
}

impl Default for LibraryFilters {
    fn default() -> Self {
        Self {
            query: None,
            source: None,
            sort_mode: SortMode::NameAsc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryScreenState {
    pub items: Vec<GameRecord>,
    pub filters: LibraryFilters,
    pub status_message: Option<String>,
    pub empty_state_hint: &'static str,
    pub primary_action_label: &'static str,
    pub help_text: &'static str,
}

pub fn load_state(filters: LibraryFilters) -> Result<LibraryScreenState> {
    let games = discovery_service::discover_and_merge_library()?;
    let items = apply_filters(games, &filters);
    Ok(LibraryScreenState {
        items,
        filters,
        status_message: None,
        empty_state_hint: "No games found yet. Add a game folder or update scan locations in Settings.",
        primary_action_label: "Add Game Folder",
        help_text: "Tip: Use a clear game name so saves are easy to identify later.",
    })
}

pub fn add_manual_game_action(name: &str, save_dir: &str, filters: LibraryFilters) -> Result<LibraryScreenState> {
    library_service::add_manual_game(name, save_dir)?;
    let mut state = load_state(filters)?;
    state.status_message = Some("Game added. You can now back up and label its saves.".to_string());
    Ok(state)
}

pub fn edit_manual_game_action(
    game_id: &str,
    new_name: &str,
    new_save_dir: &str,
    filters: LibraryFilters,
) -> Result<LibraryScreenState> {
    library_service::edit_manual_game(game_id, new_name, new_save_dir)?;
    let mut state = load_state(filters)?;
    state.status_message = Some("Game details updated successfully.".to_string());
    Ok(state)
}

fn apply_filters(mut games: Vec<GameRecord>, filters: &LibraryFilters) -> Vec<GameRecord> {
    if let Some(query) = &filters.query {
        let needle = query.to_lowercase();
        games.retain(|game| {
            game.name.to_lowercase().contains(&needle)
                || game.active_save_dir.to_string_lossy().to_lowercase().contains(&needle)
        });
    }

    if let Some(source) = &filters.source {
        games.retain(|game| &game.source == source);
    }

    match filters.sort_mode {
        SortMode::NameAsc => {
            games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        SortMode::NameDesc => {
            games.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()));
        }
        SortMode::UpdatedNewestFirst => {
            games.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
    }

    games
}
