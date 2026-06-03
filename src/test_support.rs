//! Shared helpers for tests that override process environment variables.

use std::path::Path;
use std::sync::Mutex;

static CONFIG_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Runs `f` while `SLOTFORGE_CONFIG_PATH` points at `path`, serialized across tests.
pub fn with_config_path<T, F>(path: &Path, f: F) -> T
where
    F: FnOnce() -> T,
{
    let _guard = CONFIG_ENV_LOCK
        .lock()
        .expect("config path test lock poisoned");
    let previous = std::env::var("SLOTFORGE_CONFIG_PATH").ok();
    // SAFETY: guarded by CONFIG_ENV_LOCK; restored before returning.
    unsafe {
        std::env::set_var("SLOTFORGE_CONFIG_PATH", path.to_string_lossy().as_ref());
    }
    let result = f();
    restore_config_path(previous);
    result
}

fn restore_config_path(previous: Option<String>) {
    // SAFETY: paired with `with_config_path` under the same lock.
    unsafe {
        match previous {
            Some(value) => std::env::set_var("SLOTFORGE_CONFIG_PATH", value),
            None => std::env::remove_var("SLOTFORGE_CONFIG_PATH"),
        }
    }
}
