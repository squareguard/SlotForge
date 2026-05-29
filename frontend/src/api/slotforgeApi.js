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
 * @template T
 * @param {string} command
 * @param {Record<string, unknown>} [args]
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

  /**
   * @param {{ name: string, activeSaveDir: string }} input
   */
  addGame(input) {
    return invokeApi("add_game", {
      name: input.name,
      activeSaveDir: input.activeSaveDir,
    });
  },

  /**
   * @param {{ gameId: string, label?: string | null, note?: string | null }} input
   */
  backupGame(input) {
    return invokeApi("backup_game", {
      gameId: input.gameId,
      label: input.label ?? null,
      note: input.note ?? null,
    });
  },

  /**
   * @param {{ snapshotId: string, resolutionChoice?: string, confirmedDestructive: boolean }} input
   */
  restoreSnapshot(input) {
    return invokeApi("restore_snapshot", {
      snapshotId: input.snapshotId,
      resolutionChoice: input.resolutionChoice ?? "KeepSource",
      confirmedDestructive: input.confirmedDestructive,
    });
  },

  rollbackSwap() {
    return invokeApi("rollback_swap");
  },

  /**
   * @param {{ snapshotId: string }} input
   */
  verifySnapshot(input) {
    return invokeApi("verify_snapshot", { snapshotId: input.snapshotId });
  },

  /**
   * @param {{ gameId: string }} input
   */
  verifyAllSnapshots(input) {
    return invokeApi("verify_all_snapshots", { gameId: input.gameId });
  },

  /**
   * @param {{ snapshotId: string, label?: string | null, note?: string | null }} input
   */
  updateAnnotation(input) {
    return invokeApi("update_annotation", {
      snapshotId: input.snapshotId,
      label: input.label,
      note: input.note,
    });
  },

  /**
   * @param {{ snapshotId: string, confirmed?: boolean }} input
   */
  deleteSnapshot(input) {
    return invokeApi("delete_snapshot", {
      snapshotId: input.snapshotId,
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
    return invokeApi("add_ignored_path", {
      path: input.path,
      name: input.name ?? null,
    });
  },

  /**
   * @param {{ path: string }} input
   */
  removeIgnoredPath(input) {
    return invokeApi("remove_ignored_path", { path: input.path });
  },

  /**
   * @param {{ gameId: string }} input
   */
  ignoreGameFromLibrary(input) {
    return invokeApi("ignore_game_from_library", { gameId: input.gameId });
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
 * @param {string} dirPath
 * @returns {Promise<DiscoveredSaveFile[]>}
 */
export async function scanSaveDirectory(dirPath) {
  const trimmed = dirPath.trim();
  if (!trimmed) return [];

  const res = await invokeApi("scan_save_directory", { path: trimmed });
  if (res.ok && Array.isArray(res.data)) {
    return res.data;
  }
  return [];
}

/**
 * @returns {Promise<string | null>}
 */
export async function pickSaveDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select the folder containing your game save files",
  });
  if (!selected) return null;
  if (typeof selected === "string") return selected;
  return null;
}
