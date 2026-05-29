use anyhow::{Context, Result};

use crate::domain::conflict::ConflictComparison;
use crate::domain::game::GameRecord;
use crate::domain::save::SaveRecord;
use crate::services::vault_service::{self, DeleteRequest, DeleteTarget};

pub fn title() -> &'static str {
    "Vault"
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VaultScreenState {
    pub game_name: String,
    pub saves: Vec<SaveRecord>,
    pub status_message: Option<String>,
    pub delete_confirmation_prompt: &'static str,
    pub compare_hint: &'static str,
}

pub fn load_state(game: &GameRecord) -> Result<VaultScreenState> {
    let saves = vault_service::list_vault_saves_for_game(game)?;
    Ok(VaultScreenState {
        game_name: game.name.clone(),
        saves,
        status_message: None,
        delete_confirmation_prompt:
            "Delete this save? This removes the file from your vault and cannot be undone.",
        compare_hint: "Compare two saves before replacing or deleting to avoid mistakes.",
    })
}

pub fn inspect_save(game: &GameRecord, save_id: &str) -> Result<SaveRecord> {
    let state = load_state(game)?;
    state
        .saves
        .into_iter()
        .find(|save| save.id == save_id)
        .with_context(|| format!("save '{save_id}' not found in vault"))
}

pub fn compare_saves(game: &GameRecord, source_id: &str, destination_id: &str) -> Result<ConflictComparison> {
    let source = inspect_save(game, source_id)?;
    let destination = inspect_save(game, destination_id)?;
    Ok(vault_service::compare_saves(&source, &destination))
}

pub fn annotate_save_action(
    game: &GameRecord,
    save_id: &str,
    label: Option<String>,
    note: Option<String>,
) -> Result<VaultScreenState> {
    vault_service::annotate_save(save_id, label, note, None)?;
    let mut state = load_state(game)?;
    state.status_message = Some("Save label/note updated.".to_string());
    Ok(state)
}

pub fn delete_vault_save_action(
    game: &GameRecord,
    save_id: &str,
    confirmed: bool,
) -> Result<VaultScreenState> {
    let target = inspect_save(game, save_id)?;
    vault_service::delete_save(DeleteRequest {
        save_id: target.id,
        absolute_path: target.absolute_path,
        target: DeleteTarget::Vault,
        confirmed,
        confirmation_phrase: if confirmed {
            Some("DELETE".to_string())
        } else {
            None
        },
    })?;
    let mut state = load_state(game)?;
    state.status_message = Some("Vault save deleted.".to_string());
    Ok(state)
}
