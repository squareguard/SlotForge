import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/**
 * @template T
 * @typedef {{ ok: true, data: T }} ApiOk
 */

/**
 * @typedef {{ ok: false, error: { code: string, message: string, details?: unknown } }} ApiFail
 */

/**
 * @param {unknown} err
 * @returns {string}
 */
export function unexpectedError(err) {
  return err instanceof Error ? err.message : "An unexpected error occurred.";
}

/**
 * @param {string | undefined | null} id
 * @param {string} label
 * @returns {ApiFail | null}
 */
function validationErrorIfEmpty(id, label) {
  if (id != null && String(id).trim()) {
    return null;
  }
  return { ok: false, error: { code: "VALIDATION", message: `${label} is required.` } };
}

/**
 * @template T
 * @param {string} command
 * @param {Record<string, unknown>} [argValues]
 * @returns {Promise<ApiOk<T> | ApiFail>}
 */
async function invokeApi(command, argValues = {}) {
  try {
    // Tauri commands take a single struct parameter named `args` on the Rust side;
    // the invoke payload must use that key (not flatten fields at the top level).
    const payload =
      argValues && typeof argValues === "object" && Object.keys(argValues).length > 0
        ? { args: argValues }
        : {};
    return await invoke(command, payload);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, error: { code: "IPC", message } };
  }
}

export const slotforgeApi = {
  loadLibrary() {
    return invokeApi("load_library");
  },

  scanGames() {
    return invokeApi("scan_games");
  },

  /** Full library scan on a background thread (does not block the UI). */
  scanGamesBackground() {
    return invokeApi("scan_games_background");
  },

  /**
   * @param {{ name: string, activeSaveDir: string }} input
   */
  addGame(input) {
    const name = input.name.trim();
    const activeSaveDir = input.activeSaveDir.trim();
    if (!name) {
      return Promise.resolve({
        ok: false,
        error: { code: "VALIDATION", message: "Game name is required." },
      });
    }
    if (!activeSaveDir) {
      return Promise.resolve({
        ok: false,
        error: { code: "VALIDATION", message: "Save folder path is required." },
      });
    }
    return invokeApi("add_game", { name, activeSaveDir });
  },

  /**
   * @param {{ gameId: string, label?: string | null, note?: string | null }} input
   */
  backupGame(input) {
    const err = validationErrorIfEmpty(input.gameId, "Game id");
    if (err) return Promise.resolve(err);
    return invokeApi("backup_game", {
      gameId: String(input.gameId).trim(),
      label: input.label ?? null,
      note: input.note ?? null,
    });
  },

  /**
   * Server-computed warning before overwriting active saves (see `swap_service`).
   * @param {{ snapshotId: string }} input
   */
  destructiveRestoreWarning(input) {
    const err = validationErrorIfEmpty(input.snapshotId, "Snapshot id");
    if (err) return Promise.resolve(err);
    return invokeApi("destructive_restore_warning", {
      snapshotId: String(input.snapshotId).trim(),
    });
  },

  /**
   * @param {{ snapshotId: string, resolutionChoice?: string, confirmedDestructive: boolean }} input
   * When `resolutionChoice` is omitted, Rust receives `None` (swap default policy applies).
   */
  restoreSnapshot(input) {
    const err = validationErrorIfEmpty(input.snapshotId, "Snapshot id");
    if (err) return Promise.resolve(err);
    /** @type {Record<string, unknown>} */
    const args = {
      snapshotId: String(input.snapshotId).trim(),
      confirmedDestructive: input.confirmedDestructive,
    };
    if (input.resolutionChoice != null) {
      args.resolutionChoice = input.resolutionChoice;
    }
    return invokeApi("restore_snapshot", args);
  },

  rollbackSwap() {
    return invokeApi("rollback_swap");
  },

  /**
   * @param {{ snapshotId: string }} input
   */
  verifySnapshot(input) {
    const err = validationErrorIfEmpty(input.snapshotId, "Snapshot id");
    if (err) return Promise.resolve(err);
    return invokeApi("verify_snapshot", { snapshotId: String(input.snapshotId).trim() });
  },

  /**
   * @param {{ gameId: string }} input
   */
  verifyAllSnapshots(input) {
    const err = validationErrorIfEmpty(input.gameId, "Game id");
    if (err) return Promise.resolve(err);
    return invokeApi("verify_all_snapshots", { gameId: String(input.gameId).trim() });
  },

  /**
   * @param {{ snapshotId: string, label?: string | null, note?: string | null, labelColor?: string | null }} input
   */
  updateAnnotation(input) {
    const err = validationErrorIfEmpty(input.snapshotId, "Snapshot id");
    if (err) return Promise.resolve(err);
    /** @type {Record<string, unknown>} */
    const args = { snapshotId: String(input.snapshotId).trim() };
    if ("label" in input) args.label = input.label;
    if ("note" in input) args.note = input.note;
    if ("labelColor" in input) args.labelColor = input.labelColor;
    return invokeApi("update_annotation", args);
  },

  /**
   * @param {{ snapshotId: string, confirmed?: boolean }} input
   */
  deleteSnapshot(input) {
    const err = validationErrorIfEmpty(input.snapshotId, "Snapshot id");
    if (err) return Promise.resolve(err);
    return invokeApi("delete_snapshot", {
      snapshotId: String(input.snapshotId).trim(),
      confirmed: input.confirmed ?? true,
    });
  },

  listIgnoredGames() {
    return invokeApi("list_ignored_games");
  },

  /**
   * @param {{ path: string, name?: string | null }} input
   */
  addIgnoredPath(input) {
    const path = input.path.trim();
    if (!path) {
      return Promise.resolve({
        ok: false,
        error: { code: "VALIDATION", message: "Folder path is required." },
      });
    }
    return invokeApi("add_ignored_path", {
      path,
      name: input.name ?? null,
    });
  },

  /**
   * @param {{ path: string }} input
   */
  removeIgnoredPath(input) {
    const path = input.path.trim();
    if (!path) {
      return Promise.resolve({
        ok: false,
        error: { code: "VALIDATION", message: "Folder path is required." },
      });
    }
    return invokeApi("remove_ignored_path", { path });
  },

  /**
   * @param {{ gameId: string }} input
   */
  ignoreGameFromLibrary(input) {
    const err = validationErrorIfEmpty(input.gameId, "Game id");
    if (err) return Promise.resolve(err);
    return invokeApi("ignore_game_from_library", { gameId: String(input.gameId).trim() });
  },
};

/**
 * @typedef {Object} DiscoveredSaveFile
 * @property {string} name
 * @property {string} absolutePath
 * @property {string} relativePath
 * @property {number} size
 * @property {string} modifiedAt
 */

/**
 * @typedef {{ ok: true, files: DiscoveredSaveFile[] }} ScanDirOk
 * @typedef {{ ok: false, files: [], error: string }} ScanDirFail
 */

/**
 * @param {string} dirPath
 * @returns {Promise<ScanDirOk | ScanDirFail>}
 */
export async function scanSaveDirectory(dirPath) {
  const trimmed = dirPath.trim();
  if (!trimmed) {
    return { ok: true, files: [] };
  }

  const res = await invokeApi("scan_save_directory", { path: trimmed });
  if (res.ok && Array.isArray(res.data)) {
    return { ok: true, files: res.data };
  }
  const error = res.ok ? "Invalid scan response from backend." : res.error.message;
  console.warn("scanSaveDirectory failed:", error);
  return { ok: false, files: [], error };
}

/**
 * @returns {Promise<string | null>}
 */
export async function pickSaveDirectory() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select the folder containing your game save files",
    });
    if (!selected) return null;
    if (typeof selected === "string") return selected;
    return null;
  } catch (err) {
    console.warn("pickSaveDirectory failed:", unexpectedError(err));
    throw err;
  }
}
