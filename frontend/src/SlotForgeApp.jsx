// ============================================================================
// DOMAIN TYPES — aligned with SlotForge Rust domain (src/domain/*.rs)
// ============================================================================

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { pickSaveDirectory, scanSaveDirectory, slotforgeApi, unexpectedError } from "./api/slotforgeApi.js";
import {
  GameSource,
  IntegrityStatus,
  ResolutionChoice,
  SaveFreshness,
  SaveOrigin,
} from "./domainEnums.js";
import { useSlotForgeActions } from "./hooks/useSlotForgeActions.js";
import { useSlotForgeBoot } from "./hooks/useSlotForgeBoot.js";

export { GameSource, IntegrityStatus, ResolutionChoice, SaveFreshness, SaveOrigin };
import {
  ChevronLeft,
  ChevronRight,
  EyeOff,
  FolderPlus,
  HardDrive,
  Plus,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";

/** @typedef {'AutoDiscovered' | 'UserAdded'} GameSource */

/** @typedef {'ActiveDirectory' | 'Vault'} SaveOrigin */

/** @typedef {'verified' | 'corrupted' | 'unchecked'} IntegrityStatus */

/** @typedef {'SourceNewer' | 'DestinationNewer' | 'Equal' | 'Unknown'} SaveFreshness */

/** @typedef {'KeepSource' | 'KeepDestination' | 'KeepBothRename' | 'CancelOperation'} ResolutionChoice */

/** @typedef {'scan' | 'add_game' | 'backup' | 'restore' | 'rollback' | 'verify' | 'verify_all' | 'annotate' | 'delete'} OperationType */

/** @typedef {'success' | 'failure'} OperationResult */

/**
 * Mirrors `SaveMetadata` in src/domain/save.rs
 * @typedef {Object} SaveMetadata
 * @property {string | null} modifiedAt ISO-8601 UTC
 * @property {string | null} createdAt ISO-8601 UTC
 * @property {number} byteSize
 * @property {string | null} sha256
 */

/**
 * Mirrors `SaveRecord` in src/domain/save.rs (+ UI fields)
 * @typedef {Object} Snapshot
 * @property {string} id
 * @property {string} gameId
 * @property {string} fileName
 * @property {string} absolutePath
 * @property {SaveOrigin} origin
 * @property {string | null} label
 * @property {string | null} note
 * @property {SaveMetadata} metadata
 * @property {string | null} archivedAt ISO-8601 UTC
 * @property {IntegrityStatus} integrity
 * @property {string} labelColor hex swatch for tag filter
 * @property {number} fileCount mock file count in snapshot
 * @property {string[]} files mock relative paths for detail panel
 */

/**
 * Mirrors `GameRecord` in src/domain/game.rs (+ UI fields)
 * @typedef {Object} Game
 * @property {string} id
 * @property {string} name
 * @property {string | null} gameRoot
 * @property {string} activeSaveDir
 * @property {GameSource} source
 * @property {string[]} tags
 * @property {string} createdAt ISO-8601 UTC
 * @property {string} updatedAt ISO-8601 UTC
 * @property {string | null} lastBackedUpAt ISO-8601 UTC for sidebar
 * @property {boolean} hasConflict when true, restore shows conflict modal seed
 * @property {ConflictFile[]} conflictFiles mock diff rows for conflict UI
 */

/**
 * Row in restore conflict diff view (UI mock; maps to comparison UX)
 * @typedef {Object} ConflictFile
 * @property {string} path
 * @property {SaveFreshness} freshness
 * @property {string} activeSnippet mock diff excerpt for active save
 * @property {string} snapshotSnippet mock diff excerpt for vault snapshot
 */

/**
 * Mirrors `ConflictComparison` in src/domain/conflict.rs
 * @typedef {Object} ConflictComparison
 * @property {string} sourcePath
 * @property {string} destinationPath
 * @property {SaveMetadata} sourceMetadata
 * @property {SaveMetadata} destinationMetadata
 * @property {SaveFreshness} freshness
 * @property {string} reason
 */

/**
 * Last swap state for rollback UI
 * @typedef {Object} LastSwap
 * @property {string} gameId
 * @property {string} snapshotId vault snapshot that was restored
 * @property {string} previousActivePath mock path staged before swap
 * @property {string} restoredAt ISO-8601 UTC
 */

/**
 * Status bar + audit-style operation log entry
 * @typedef {Object} OperationLog
 * @property {string} id
 * @property {OperationType} type
 * @property {OperationResult} result
 * @property {string} message
 * @property {string} timestamp ISO-8601 UTC
 * @property {string | null} gameId
 * @property {string | null} snapshotId
 */

/**
 * Save file discovered on disk (dev scan API / add-game flow).
 * @typedef {Object} DiscoveredSaveFile
 * @property {string} name
 * @property {string} absolutePath
 * @property {string} relativePath
 * @property {number} size
 * @property {string} modifiedAt ISO-8601
 */

/** Label colour swatches for snapshot tags (vault filter + colour picker). */
const LABEL_COLORS = ["#00f5ff", "#ffb800", "#a855f7", "#22c55e", "#ff2d55", "#f472b6"];

/** @param {string} id */
function labelColorForId(id) {
  let hash = 0;
  for (let i = 0; i < id.length; i++) {
    hash = (hash + id.charCodeAt(i)) >>> 0;
  }
  return LABEL_COLORS[hash % LABEL_COLORS.length];
}

const GAME_SOURCES = new Set(Object.values(GameSource));
const SAVE_ORIGINS = new Set(Object.values(SaveOrigin));
const INTEGRITY_STATUSES = new Set(Object.values(IntegrityStatus));
const SAVE_FRESHNESS_VALUES = new Set(Object.values(SaveFreshness));

/** @param {unknown} value @returns {value is GameSource} */
export function isGameSource(value) {
  return typeof value === "string" && GAME_SOURCES.has(value);
}

/** @param {unknown} value @returns {value is SaveOrigin} */
export function isSaveOrigin(value) {
  return typeof value === "string" && SAVE_ORIGINS.has(value);
}

/** @param {unknown} value @returns {value is IntegrityStatus} */
export function isIntegrityStatus(value) {
  return typeof value === "string" && INTEGRITY_STATUSES.has(value);
}

/** @param {unknown} value @returns {value is SaveFreshness} */
export function isSaveFreshness(value) {
  return typeof value === "string" && SAVE_FRESHNESS_VALUES.has(value);
}

/**
 * @param {SaveMetadata} metadata
 * @returns {boolean}
 */
export function isValidSaveMetadata(metadata) {
  return (
    metadata != null &&
    typeof metadata === "object" &&
    typeof metadata.byteSize === "number" &&
    metadata.byteSize >= 0 &&
    (metadata.modifiedAt === null || typeof metadata.modifiedAt === "string") &&
    (metadata.createdAt === null || typeof metadata.createdAt === "string") &&
    (metadata.sha256 === null || typeof metadata.sha256 === "string")
  );
}

/**
 * @param {Game} game
 * @returns {boolean}
 */
export function isValidGame(game) {
  return (
    game != null &&
    typeof game.id === "string" &&
    typeof game.name === "string" &&
    typeof game.activeSaveDir === "string" &&
    isGameSource(game.source) &&
    Array.isArray(game.tags) &&
    typeof game.createdAt === "string" &&
    typeof game.updatedAt === "string" &&
    typeof game.hasConflict === "boolean" &&
    Array.isArray(game.conflictFiles)
  );
}

/**
 * @param {Snapshot} snapshot
 * @returns {boolean}
 */
export function isValidSnapshot(snapshot) {
  return (
    snapshot != null &&
    typeof snapshot.id === "string" &&
    typeof snapshot.gameId === "string" &&
    typeof snapshot.fileName === "string" &&
    typeof snapshot.absolutePath === "string" &&
    isSaveOrigin(snapshot.origin) &&
    isIntegrityStatus(snapshot.integrity) &&
    typeof snapshot.labelColor === "string" &&
    typeof snapshot.fileCount === "number" &&
    Array.isArray(snapshot.files) &&
    isValidSaveMetadata(snapshot.metadata)
  );
}

/**
 * @param {OperationLog} entry
 * @returns {boolean}
 */
export function isValidOperationLog(entry) {
  return (
    entry != null &&
    typeof entry.id === "string" &&
    typeof entry.type === "string" &&
    (entry.result === "success" || entry.result === "failure") &&
    typeof entry.message === "string" &&
    typeof entry.timestamp === "string"
  );
}


// ============================================================================
// APP STATE (useReducer) — task 2.6
// ============================================================================

/**
 * @typedef {Object} UiState
 * @property {{ sidebarCollapsed: boolean, detailCollapsed: boolean }} panels
 * @property {{ addGameOpen: boolean, backupOpen: boolean, restoreOpen: boolean, deleteOpen: boolean }} modals
 * @property {{ scanning: boolean, addingGame: boolean, backingUp: boolean, restoring: boolean, rollingBack: boolean, verifying: boolean, batchVerifying: boolean, deleting: boolean }} loading
 */

/**
 * @typedef {Object} ProgressState
 * @property {OperationType | null} type
 * @property {number} current
 * @property {number} total
 * @property {string | null} message
 */

/**
 * @typedef {Object} OperationsState
 * @property {OperationLog | null} lastOp
 * @property {LastSwap | null} lastSwap
 * @property {ProgressState} progress
 */

/**
 * @typedef {Object} AppState
 * @property {Game[]} games
 * @property {Record<string, Snapshot[]>} vaultByGameId
 * @property {string | null} selectedGameId
 * @property {string | null} selectedSnapshotId
 * @property {UiState} ui
 * @property {OperationsState} operations
 * @property {boolean} settingsViewOpen
 */

/**
 * @typedef {Object} AppAction
 * @property {string} type
 * @property {any=} payload
 */

/**
 * @typedef {Object} LibraryDb
 * @property {Game[]} games
 * @property {Record<string, Snapshot[]>} vaultByGameId
 * @property {LastSwap | null} lastSwap
 */

/**
 * @param {LibraryDb} db
 * @returns {AppState}
 */
export function makeInitialAppState(db) {
  const selectedGameId = db.games[0]?.id ?? null;
  const firstSnapshot = selectedGameId ? (db.vaultByGameId[selectedGameId]?.[0] ?? null) : null;

  return {
    games: db.games,
    vaultByGameId: db.vaultByGameId,
    selectedGameId,
    selectedSnapshotId: firstSnapshot?.id ?? null,
    ui: {
      panels: { sidebarCollapsed: false, detailCollapsed: false },
      modals: { addGameOpen: false, backupOpen: false, restoreOpen: false, deleteOpen: false },
      loading: {
        scanning: false,
        addingGame: false,
        backingUp: false,
        restoring: false,
        rollingBack: false,
        verifying: false,
        batchVerifying: false,
        deleting: false,
      },
    },
    operations: {
      lastOp: null,
      lastSwap: db.lastSwap ?? null,
      progress: { type: null, current: 0, total: 0, message: null },
    },
    settingsViewOpen: false,
  };
}

/**
 * Reducer for all app state slices (task 2.6).
 * UI wiring comes in task 2.7+.
 *
 * @param {AppState} state
 * @param {AppAction} action
 * @returns {AppState}
 */
export function appReducer(state, action) {
  switch (action.type) {
    case "SET_DB": {
      /** @type {LibraryDb} */
      const db = action.payload;
      const nextSelectedGameId = state.selectedGameId ?? db.games[0]?.id ?? null;
      const nextSelectedSnapshotId =
        state.selectedSnapshotId ??
        (nextSelectedGameId ? (db.vaultByGameId[nextSelectedGameId]?.[0]?.id ?? null) : null);

      return {
        ...state,
        games: db.games,
        vaultByGameId: db.vaultByGameId,
        selectedGameId: nextSelectedGameId,
        selectedSnapshotId: nextSelectedSnapshotId,
        operations: {
          ...state.operations,
          lastSwap: db.lastSwap ?? null,
        },
      };
    }

    case "SELECT_GAME": {
      const gameId = action.payload ?? null;
      const firstSnapshot = gameId ? (state.vaultByGameId[gameId]?.[0] ?? null) : null;
      return {
        ...state,
        selectedGameId: gameId,
        selectedSnapshotId: firstSnapshot?.id ?? null,
      };
    }

    case "SELECT_SNAPSHOT": {
      return { ...state, selectedSnapshotId: action.payload ?? null };
    }

    case "TOGGLE_PANEL": {
      const which = action.payload;
      if (which !== "sidebar" && which !== "detail") return state;
      return {
        ...state,
        ui: {
          ...state.ui,
          panels: {
            ...state.ui.panels,
            sidebarCollapsed:
              which === "sidebar" ? !state.ui.panels.sidebarCollapsed : state.ui.panels.sidebarCollapsed,
            detailCollapsed:
              which === "detail" ? !state.ui.panels.detailCollapsed : state.ui.panels.detailCollapsed,
          },
        },
      };
    }

    case "OPEN_MODAL": {
      const name = action.payload;
      if (!name) return state;
      return {
        ...state,
        ui: {
          ...state.ui,
          modals: {
            ...state.ui.modals,
            addGameOpen: name === "addGame" ? true : state.ui.modals.addGameOpen,
            backupOpen: name === "backup" ? true : state.ui.modals.backupOpen,
            restoreOpen: name === "restore" ? true : state.ui.modals.restoreOpen,
            deleteOpen: name === "delete" ? true : state.ui.modals.deleteOpen,
          },
        },
      };
    }

    case "CLOSE_MODAL": {
      const name = action.payload;
      if (!name) return state;
      return {
        ...state,
        ui: {
          ...state.ui,
          modals: {
            ...state.ui.modals,
            addGameOpen: name === "addGame" ? false : state.ui.modals.addGameOpen,
            backupOpen: name === "backup" ? false : state.ui.modals.backupOpen,
            restoreOpen: name === "restore" ? false : state.ui.modals.restoreOpen,
            deleteOpen: name === "delete" ? false : state.ui.modals.deleteOpen,
          },
        },
      };
    }

    case "SET_LOADING": {
      const { key, value } = action.payload ?? {};
      if (typeof key !== "string" || typeof value !== "boolean") return state;
      if (!(key in state.ui.loading)) return state;
      return {
        ...state,
        ui: {
          ...state.ui,
          loading: { ...state.ui.loading, [key]: value },
        },
      };
    }

    case "SET_LAST_OP": {
      return {
        ...state,
        operations: {
          ...state.operations,
          lastOp: action.payload ?? null,
        },
      };
    }

    case "SET_LAST_SWAP": {
      return {
        ...state,
        operations: {
          ...state.operations,
          lastSwap: action.payload ?? null,
        },
      };
    }

    case "SET_PROGRESS": {
      return {
        ...state,
        operations: {
          ...state.operations,
          progress: {
            ...state.operations.progress,
            ...(action.payload ?? {}),
          },
        },
      };
    }

    case "SET_SETTINGS_OPEN": {
      return { ...state, settingsViewOpen: Boolean(action.payload) };
    }

    case "PATCH_SNAPSHOT": {
      const { gameId, snapshotId, patch } = action.payload ?? {};
      if (!gameId || !snapshotId || !patch) return state;
      const list = state.vaultByGameId[gameId];
      if (!list) return state;
      return {
        ...state,
        vaultByGameId: {
          ...state.vaultByGameId,
          [gameId]: list.map((s) => (s.id === snapshotId ? { ...s, ...patch } : s)),
        },
      };
    }

    default:
      return state;
  }
}

// ============================================================================
// THEME — ThemeContext + reducer (task 3.3)
// ============================================================================

/**
 * @typedef {Object} ThemeTokens
 * @property {string} accent
 * @property {string} bgPrimary
 * @property {string} bgPanel
 * @property {string} textPrimary
 * @property {string} textDim
 * @property {string} danger
 * @property {string} warning
 */

/**
 * @typedef {Object} ThemePreset
 * @property {string} name
 * @property {ThemeTokens} tokens
 * @property {number} fontSize
 * @property {'compact' | 'comfortable'} density
 * @property {boolean} scanlinesEnabled
 * @property {boolean} glowEnabled
 */

/**
 * @typedef {Object} ThemeState
 * @property {string} presetName
 * @property {ThemeTokens} tokens
 * @property {number} fontSize
 * @property {'compact' | 'comfortable'} density
 * @property {boolean} scanlinesEnabled
 * @property {boolean} glowEnabled
 */

/** @type {ThemePreset} */
export const THEME_PRESET_DARKROOM = {
  name: "DARKROOM",
  tokens: {
    accent: "#00f5ff",
    bgPrimary: "#0a0a0f",
    bgPanel: "#12121a",
    textPrimary: "#e8f4f8",
    textDim: "#6b7a8f",
    danger: "#ff2d55",
    warning: "#ffb800",
  },
  fontSize: 16,
  density: "comfortable",
  scanlinesEnabled: false,
  glowEnabled: false,
};

/** @type {ThemePreset} */
export const THEME_PRESET_MATRIX = {
  name: "MATRIX",
  tokens: {
    accent: "#39ff14",
    bgPrimary: "#050805",
    bgPanel: "#0b120b",
    textPrimary: "#d7ffd9",
    textDim: "#5f7a62",
    danger: "#ff2d55",
    warning: "#ffb800",
  },
  fontSize: 16,
  density: "comfortable",
  scanlinesEnabled: true,
  glowEnabled: false,
};

/** @type {ThemePreset} */
export const THEME_PRESET_VOID = {
  name: "VOID",
  tokens: {
    accent: "#8b5cf6",
    bgPrimary: "#07060c",
    bgPanel: "#110f18",
    textPrimary: "#ddd6fe",
    textDim: "#6b6280",
    danger: "#ff2d55",
    warning: "#ffb800",
  },
  fontSize: 16,
  density: "comfortable",
  scanlinesEnabled: false,
  glowEnabled: false,
};

/** @type {ThemePreset} */
export const THEME_PRESET_NEON_TOKYO = {
  name: "NEON TOKYO",
  tokens: {
    accent: "#ff2fd6",
    bgPrimary: "#0a0610",
    bgPanel: "#140a1c",
    textPrimary: "#f5e6ff",
    textDim: "#8a6f9b",
    danger: "#ff2d55",
    warning: "#ffb800",
  },
  fontSize: 16,
  density: "comfortable",
  scanlinesEnabled: true,
  glowEnabled: false,
};

/** @type {ThemePreset} */
export const THEME_PRESET_BLOODLINE = {
  name: "BLOODLINE",
  tokens: {
    accent: "#ff2d55",
    bgPrimary: "#0f0608",
    bgPanel: "#1a0c10",
    textPrimary: "#ffe8ec",
    textDim: "#9a6b74",
    danger: "#ff2d55",
    warning: "#ffb800",
  },
  fontSize: 16,
  density: "comfortable",
  scanlinesEnabled: false,
  glowEnabled: false,
};

/** @type {Record<string, ThemePreset>} */
export const THEME_PRESETS = {
  DARKROOM: THEME_PRESET_DARKROOM,
  MATRIX: THEME_PRESET_MATRIX,
  VOID: THEME_PRESET_VOID,
  "NEON TOKYO": THEME_PRESET_NEON_TOKYO,
  BLOODLINE: THEME_PRESET_BLOODLINE,
};

/** @returns {ThemeState} */
function themeStateFromPreset(preset) {
  return {
    presetName: preset.name,
    tokens: { ...preset.tokens },
    fontSize: preset.fontSize,
    density: preset.density,
    scanlinesEnabled: preset.scanlinesEnabled,
    glowEnabled: preset.glowEnabled,
  };
}

const initialThemeState = themeStateFromPreset(THEME_PRESET_DARKROOM);

/**
 * @param {ThemeState} theme
 */
function applyThemeToDocument(theme) {
  const root = document.documentElement;
  root.style.setProperty("--accent", theme.tokens.accent);
  root.style.setProperty("--bg-primary", theme.tokens.bgPrimary);
  root.style.setProperty("--bg-panel", theme.tokens.bgPanel);
  root.style.setProperty("--text-primary", theme.tokens.textPrimary);
  root.style.setProperty("--text-dim", theme.tokens.textDim);
  root.style.setProperty("--danger", theme.tokens.danger);
  root.style.setProperty("--warning", theme.tokens.warning);
  root.style.setProperty("--font-size-base", `${theme.fontSize}px`);
  root.style.setProperty("--panel-density", theme.density);
  root.style.setProperty("--scanlines-enabled", theme.scanlinesEnabled ? "1" : "0");
  root.style.setProperty("--glow-enabled", theme.glowEnabled ? "1" : "0");
  root.dataset.theme = theme.presetName;
  root.dataset.density = theme.density;
  root.dataset.glow = theme.glowEnabled ? "1" : "0";
  root.dataset.scanlines = theme.scanlinesEnabled ? "1" : "0";
}

/**
 * @param {ThemeState} state
 * @param {{ type: string, payload?: any }} action
 * @returns {ThemeState}
 */
function themeReducer(state, action) {
  switch (action.type) {
    case "APPLY_PRESET": {
      return themeStateFromPreset(action.payload);
    }
    case "SET_TOKEN": {
      const { key, value } = action.payload ?? {};
      if (!key || typeof value !== "string") return state;
      if (!(key in state.tokens)) return state;
      return {
        ...state,
        presetName: "Custom",
        tokens: { ...state.tokens, [key]: value },
      };
    }
    case "SET_DENSITY": {
      const density = action.payload === "compact" ? "compact" : "comfortable";
      return { ...state, presetName: "Custom", density };
    }
    case "SET_FONT_SIZE": {
      const size = Number(action.payload);
      if (!Number.isFinite(size) || size < 12 || size > 22) return state;
      return { ...state, presetName: "Custom", fontSize: size };
    }
    case "TOGGLE_SCANLINES": {
      return {
        ...state,
        presetName: "Custom",
        scanlinesEnabled: !state.scanlinesEnabled,
      };
    }
    case "TOGGLE_GLOW": {
      return {
        ...state,
        presetName: "Custom",
        glowEnabled: !state.glowEnabled,
      };
    }
    case "IMPORT_THEME": {
      return action.payload;
    }
    default:
      return state;
  }
}

/**
 * @typedef {Object} ThemeContextValue
 * @property {ThemeState} theme
 * @property {(preset: ThemePreset) => void} applyPreset
 * @property {(key: keyof ThemeTokens, value: string) => void} setToken
 * @property {(density: 'compact' | 'comfortable') => void} setDensity
 * @property {(size: number) => void} setFontSize
 * @property {() => void} toggleScanlines
 * @property {() => void} toggleGlow
 * @property {() => string} exportTheme
 * @property {(json: string) => { ok: true } | { ok: false, error: string }} importTheme
 */

/** @type {import('react').Context<ThemeContextValue | null>} */
const ThemeContext = createContext(null);

/**
 * @param {{ children: import('react').ReactNode }} props
 */
export function ThemeProvider({ children }) {
  const [theme, dispatch] = useReducer(themeReducer, initialThemeState);

  useEffect(() => {
    applyThemeToDocument(theme);
  }, [theme]);

  const applyPreset = useCallback((preset) => {
    dispatch({ type: "APPLY_PRESET", payload: preset });
  }, []);

  const setToken = useCallback((key, value) => {
    dispatch({ type: "SET_TOKEN", payload: { key, value } });
  }, []);

  const setDensity = useCallback((density) => {
    dispatch({ type: "SET_DENSITY", payload: density });
  }, []);

  const setFontSize = useCallback((size) => {
    dispatch({ type: "SET_FONT_SIZE", payload: size });
  }, []);

  const toggleScanlines = useCallback(() => {
    dispatch({ type: "TOGGLE_SCANLINES" });
  }, []);

  const toggleGlow = useCallback(() => {
    dispatch({ type: "TOGGLE_GLOW" });
  }, []);

  const exportTheme = useCallback(() => {
    return JSON.stringify(
      {
        version: 1,
        presetName: theme.presetName,
        tokens: theme.tokens,
        fontSize: theme.fontSize,
        density: theme.density,
        scanlinesEnabled: theme.scanlinesEnabled,
        glowEnabled: theme.glowEnabled,
      },
      null,
      2
    );
  }, [theme]);

  const importTheme = useCallback((json) => {
    try {
      const parsed = JSON.parse(json);
      if (!parsed || typeof parsed !== "object") {
        return { ok: false, error: "Theme JSON must be an object." };
      }
      const tokens = parsed.tokens;
      if (!tokens || typeof tokens !== "object") {
        return { ok: false, error: "Missing tokens object." };
      }
      const required = [
        "accent",
        "bgPrimary",
        "bgPanel",
        "textPrimary",
        "textDim",
        "danger",
        "warning",
      ];
      for (const key of required) {
        if (typeof tokens[key] !== "string") {
          return { ok: false, error: `Invalid or missing token: ${key}` };
        }
      }
      const next = {
        presetName: typeof parsed.presetName === "string" ? parsed.presetName : "Imported",
        tokens: {
          accent: tokens.accent,
          bgPrimary: tokens.bgPrimary,
          bgPanel: tokens.bgPanel,
          textPrimary: tokens.textPrimary,
          textDim: tokens.textDim,
          danger: tokens.danger,
          warning: tokens.warning,
        },
        fontSize:
          typeof parsed.fontSize === "number" && parsed.fontSize >= 12 && parsed.fontSize <= 22
            ? parsed.fontSize
            : 16,
        density: parsed.density === "compact" ? "compact" : "comfortable",
        scanlinesEnabled: Boolean(parsed.scanlinesEnabled),
        glowEnabled: parsed.glowEnabled !== false,
      };
      dispatch({ type: "IMPORT_THEME", payload: next });
      return { ok: true };
    } catch {
      return { ok: false, error: "Invalid JSON." };
    }
  }, []);

  const value = useMemo(
    () => ({
      theme,
      applyPreset,
      setToken,
      setDensity,
      setFontSize,
      toggleScanlines,
      toggleGlow,
      exportTheme,
      importTheme,
    }),
    [
      theme,
      applyPreset,
      setToken,
      setDensity,
      setFontSize,
      toggleScanlines,
      toggleGlow,
      exportTheme,
      importTheme,
    ]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

/** @returns {ThemeContextValue} */
export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}

// ============================================================================
// UI COMPONENTS (tasks 3.5–4.8)
// ============================================================================

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function formatWhen(iso) {
  if (!iso) return "Never";
  return new Date(iso).toLocaleString();
}

function gameInitials(name) {
  return name
    .split(/\s+/)
    .map((w) => w[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

const ToastContext = createContext(null);

function ToastProvider({ children }) {
  const [toasts, setToasts] = useState(/** @type {{ id: string, type: string, message: string }[]} */ ([]));

  const pushToast = useCallback(({ type, message }) => {
    const id = `toast-${Date.now()}`;
    setToasts((prev) => [...prev, { id, type, message }]);
    window.setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 5000);
  }, []);

  const dismissToast = useCallback((id) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return (
    <ToastContext.Provider value={{ pushToast }}>
      {children}
      <ToastSystem toasts={toasts} onDismiss={dismissToast} />
    </ToastContext.Provider>
  );
}

function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within ToastProvider");
  return ctx;
}

function toastSurfaceClass(type) {
  if (type === "error") return "border-danger/50 text-danger";
  if (type === "success") return "border-success/50 text-success";
  return "border-white/20 text-text-primary";
}

function ToastSystem({ toasts, onDismiss }) {
  return (
    <div className="pointer-events-none fixed bottom-12 right-4 z-[60] flex max-w-sm flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={[
            "toast-item pointer-events-auto flex items-start justify-between gap-3 rounded-lg border bg-bg-panel px-3 py-2 font-mono text-sm",
            toastSurfaceClass(t.type),
          ].join(" ")}
        >
          <span>{t.message}</span>
          <button
            type="button"
            onClick={() => onDismiss(t.id)}
            aria-label="Dismiss"
            className="ui-focus-ring shrink-0 rounded p-0.5"
          >
            <X size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}

function StatusBar({ lastOp, totalVaultBytes, activeGameName }) {
  return (
    <footer className="flex h-10 shrink-0 items-center justify-between border-t border-white/10 bg-bg-panel px-4 font-mono text-xs text-text-dim">
      <span>
        {lastOp ? `${lastOp.type} · ${lastOp.result} · ${formatWhen(lastOp.timestamp)}` : "Ready"}
      </span>
      <span className="flex gap-4">
        <span className="flex items-center gap-1">
          <HardDrive size={16} /> {formatBytes(totalVaultBytes)}
        </span>
        <span>Active: {activeGameName ?? "—"}</span>
      </span>
    </footer>
  );
}

function IntegrityBadge({ integrity, loading }) {
  const label =
    integrity === IntegrityStatus.Verified
      ? "Verified ✓"
      : integrity === IntegrityStatus.Corrupted
        ? "Corrupted ✗"
        : "Unchecked";
  return (
    <span
      className={[
        "rounded-md border px-2 py-0.5 font-mono text-xs",
        integrity === IntegrityStatus.Verified
          ? "border-success/40 text-success"
          : integrity === IntegrityStatus.Corrupted
            ? "border-danger/40 text-danger"
            : "border-white/15 text-text-dim",
        loading ? "opacity-50" : "",
      ].join(" ")}
    >
      {loading ? "…" : label}
    </span>
  );
}

function ProgressBar({ progress }) {
  const pct = Math.max(0, Math.min(100, progress));
  return (
    <div className="h-2 w-full rounded-md bg-black/40">
      <div className="h-full bg-accent transition-[width] duration-200 ease-out" style={{ width: `${pct}%` }} />
    </div>
  );
}

function AppShell({ sidebar, main, detail, statusBar, scanlines, overlay }) {
  return (
    <div className="relative flex h-screen min-w-[1280px] flex-col bg-bg-primary text-text-primary">
      {scanlines ? <div className="scanline-overlay" aria-hidden /> : null}
      <div className="flex min-h-0 flex-1">
        {sidebar}
        {main}
        {detail}
      </div>
      {statusBar}
      {overlay}
    </div>
  );
}

/**
 * @typedef {Object} IgnoredEntry
 * @property {string} path
 * @property {string | null} name
 * @property {string} ignoredAt ISO-8601 UTC
 */

function GameContextMenu({ x, y, gameName, onDelete, onClose }) {
  const menuRef = useRef(/** @type {HTMLDivElement | null} */ (null));

  useEffect(() => {
    const handlePointer = (e) => {
      if (menuRef.current && !menuRef.current.contains(e.target)) {
        onClose();
      }
    };
    const handleKey = (e) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", handlePointer);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointer);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      role="menu"
      className="fixed z-50 min-w-[10rem] rounded-lg border border-white/15 bg-bg-panel py-1 shadow-elevation"
      style={{ left: x, top: y }}
    >
      <button
        type="button"
        role="menuitem"
        onClick={onDelete}
        className="ui-menu-item text-danger hover:bg-danger/10"
      >
        <Trash2 size={16} />
        Delete
      </button>
      <p className="border-t border-white/10 px-3 py-2 font-mono text-xs text-text-dim">
        Removes &quot;{gameName}&quot; from SlotForge only. Save files on disk are not deleted.
      </p>
    </div>
  );
}

function GameSidebar({
  games,
  query,
  onQuery,
  selectedId,
  onSelect,
  onScan,
  scanning,
  onAdd,
  onSettings,
  onGameContextMenu,
}) {
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? games.filter((g) => g.name.toLowerCase().includes(q)) : games;
  }, [games, query]);

  return (
    <aside className="flex h-full min-h-0 w-64 shrink-0 flex-col overflow-hidden border-r border-white/10 bg-bg-panel">
      <div className="density-pad shrink-0 border-b border-white/10">
        <div className="flex flex-col items-start gap-1">
          <img
            src="/sqg_logo.png"
            alt="SquareGuard"
            className="h-2.5 w-auto max-w-full object-contain object-left opacity-90"
          />
          <h1 className="font-display text-2xl font-semibold text-accent">SlotForge</h1>
        </div>
        <div className="mt-3 flex gap-2">
          <button
            type="button"
            onClick={onScan}
            disabled={scanning}
            className="ui-btn flex flex-1 items-center justify-center gap-1 border-accent/40 py-1.5 text-accent disabled:opacity-50"
          >
            <RefreshCw size={16} className={scanning ? "animate-spin" : ""} /> Scan
          </button>
          <button type="button" onClick={onAdd} className="ui-icon-btn" aria-label="Add game">
            <Plus size={16} />
          </button>
          <button type="button" onClick={onSettings} className="ui-icon-btn" aria-label="Settings">
            <Settings size={16} />
          </button>
        </div>
        <div className="relative mt-3">
          <Search size={16} className="absolute left-2 top-2 text-text-dim" />
          <input
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder="Filter games…"
            className="ui-input w-full py-2 pl-8"
          />
        </div>
      </div>
      <div className="scroll-y-panel min-h-0 flex-1 p-2" role="list" aria-label="Games">
        {filtered.length === 0 ? (
          <p className="px-1 py-2 font-mono text-xs text-text-dim">No games match your filter.</p>
        ) : null}
        {filtered.map((g) => (
          <button
            key={g.id}
            type="button"
            onClick={() => onSelect(g.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              onGameContextMenu(g, e);
            }}
            className={[
              "ui-panel-interactive mb-2 flex w-full items-center gap-3 p-2 text-left",
              g.id === selectedId ? "is-active border-accent/60 bg-accent/5" : "border-white/10",
            ].join(" ")}
          >
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md border border-white/10 bg-accent/10 font-display text-sm font-semibold text-accent">
              {gameInitials(g.name)}
            </div>
            <div className="min-w-0">
              <div className="truncate font-display text-sm font-semibold">{g.name}</div>
              <div className="font-mono text-xs text-text-dim">Backup: {formatWhen(g.lastBackedUpAt)}</div>
            </div>
          </button>
        ))}
      </div>
    </aside>
  );
}

function VaultBrowser({
  game,
  snapshots,
  sort,
  onSort,
  labelFilter,
  onLabelFilter,
  integrityFilter,
  onIntegrityFilter,
  colorFilter,
  onColorFilter,
  selectedSnapshotId,
  onSelectSnapshot,
  onBackup,
  onVerifyAll,
  batchVerifying,
  onVerifySnapshot,
  onAnnotation,
  verifyingId,
  onRescanActive,
  rescanningActive,
}) {
  const sorted = useMemo(() => {
    let list = [...snapshots];
    if (labelFilter.trim()) {
      const q = labelFilter.toLowerCase();
      list = list.filter((s) => (s.label ?? "").toLowerCase().includes(q));
    }
    if (integrityFilter !== "all") list = list.filter((s) => s.integrity === integrityFilter);
    if (colorFilter !== "all") list = list.filter((s) => s.labelColor === colorFilter);
    list.sort((a, b) => {
      if (sort === "label") return (a.label ?? a.fileName).localeCompare(b.label ?? b.fileName);
      if (sort === "integrity") return a.integrity.localeCompare(b.integrity);
      const ta = new Date(a.metadata.modifiedAt ?? 0).getTime();
      const tb = new Date(b.metadata.modifiedAt ?? 0).getTime();
      return sort === "date-asc" ? ta - tb : tb - ta;
    });
    return list;
  }, [snapshots, sort, labelFilter, integrityFilter, colorFilter]);

  const activeList = sorted.filter((s) => s.origin === SaveOrigin.ActiveDirectory);
  const vaultList = sorted.filter((s) => s.origin === SaveOrigin.Vault);

  return (
    <main className="flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="density-pad flex shrink-0 items-center justify-between border-b border-white/10">
        <h2 className="font-display text-xl font-semibold">
          {game ? `${game.name} Vault` : "Vault"}
        </h2>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={!game || rescanningActive}
            onClick={onRescanActive}
            className="ui-btn border-white/15 px-3 py-1 disabled:opacity-40"
          >
            {rescanningActive ? "Scanning…" : "Rescan folder"}
          </button>
          <button
            type="button"
            disabled={!game || batchVerifying}
            onClick={onVerifyAll}
            className="ui-btn border-white/15 px-3 py-1 disabled:opacity-40"
          >
            {batchVerifying ? "Verifying…" : "Verify all"}
          </button>
          <button
            type="button"
            disabled={!game}
            onClick={onBackup}
            className="ui-btn border-accent/50 px-3 py-1 text-accent disabled:opacity-40"
          >
            Backup Now
          </button>
        </div>
      </div>
      <div className="density-pad flex shrink-0 flex-wrap gap-2 border-b border-white/10 font-mono text-xs">
        <select value={sort} onChange={(e) => onSort(e.target.value)} className="ui-select">
          <option value="date-desc">Newest</option>
          <option value="date-asc">Oldest</option>
          <option value="label">Label</option>
          <option value="integrity">Integrity</option>
        </select>
        <input
          value={labelFilter}
          onChange={(e) => onLabelFilter(e.target.value)}
          placeholder="Label"
          className="ui-select"
        />
        <select
          value={integrityFilter}
          onChange={(e) => onIntegrityFilter(e.target.value)}
          className="ui-select"
        >
          <option value="all">All</option>
          <option value="verified">Verified</option>
          <option value="corrupted">Corrupted</option>
          <option value="unchecked">Unchecked</option>
        </select>
        <select
          value={colorFilter}
          onChange={(e) => onColorFilter(e.target.value)}
          className="ui-select"
        >
          <option value="all">All colours</option>
          {LABEL_COLORS.map((c) => (
            <option key={c} value={c}>
              ● {c}
            </option>
          ))}
        </select>
      </div>
      <div className="scroll-y-panel min-h-0 flex-1 p-3">
        {sorted.length === 0 ? (
          <p className="font-mono text-sm text-text-dim">
            {game
              ? "No save files found. Use Browse when adding a game, or run Backup Now to create vault copies."
              : "Select a game from the sidebar."}
          </p>
        ) : null}
        {activeList.length > 0 ? (
          <>
            <h3 className="mb-2 font-mono text-xs uppercase tracking-wider text-accent">Active saves</h3>
            {activeList.map((snap) => (
              <SnapshotCard
                key={snap.id}
                snapshot={snap}
                selected={snap.id === selectedSnapshotId}
                onSelect={() => onSelectSnapshot(snap.id)}
                onVerify={() => onVerifySnapshot(snap.id)}
                verifying={verifyingId === snap.id}
                onAnnotation={onAnnotation}
              />
            ))}
          </>
        ) : null}
        {vaultList.length > 0 ? (
          <>
            <h3 className="mb-2 mt-4 font-mono text-xs uppercase tracking-wider text-text-dim">Vault backups</h3>
            {vaultList.map((snap) => (
              <SnapshotCard
                key={snap.id}
                snapshot={snap}
                selected={snap.id === selectedSnapshotId}
                onSelect={() => onSelectSnapshot(snap.id)}
                onVerify={() => onVerifySnapshot(snap.id)}
                verifying={verifyingId === snap.id}
                onAnnotation={onAnnotation}
              />
            ))}
          </>
        ) : null}
      </div>
    </main>
  );
}

function SnapshotCard({ snapshot, selected, onSelect, onVerify, verifying, onAnnotation }) {
  const [editing, setEditing] = useState(false);
  const [labelDraft, setLabelDraft] = useState(snapshot.label ?? "");
  const [noteDraft, setNoteDraft] = useState(snapshot.note ?? "");
  const [expanded, setExpanded] = useState(false);

  const saveAnnotation = () => {
    onAnnotation(snapshot.id, snapshot.gameId, {
      label: labelDraft || null,
      note: noteDraft || null,
    });
    setEditing(false);
  };

  return (
    <div
      className={[
        "ui-panel-interactive mb-3 w-full border-l-2 p-3 text-left",
        selected ? "is-active border-accent/50" : "border-white/10",
      ].join(" ")}
      style={{ borderLeftColor: snapshot.labelColor }}
    >
      <button type="button" onClick={onSelect} className="w-full text-left">
        <div className="flex justify-between gap-2">
          <div>
            {editing ? (
              <input
                value={labelDraft}
                onChange={(e) => setLabelDraft(e.target.value)}
                onClick={(e) => e.stopPropagation()}
                className="ui-input w-full px-2 py-1 font-display text-sm"
              />
            ) : (
              <div
                className="font-display text-sm font-semibold"
                style={{ color: snapshot.labelColor }}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  setEditing(true);
                }}
              >
                {snapshot.label ?? snapshot.fileName}
              </div>
            )}
            <div className="font-mono text-xs text-text-dim">{formatWhen(snapshot.metadata.modifiedAt)}</div>
          </div>
          <div className="flex flex-col items-end gap-1">
            <span
              className={[
                "rounded-md border px-1.5 py-0.5 font-mono text-xs uppercase",
                snapshot.origin === SaveOrigin.ActiveDirectory
                  ? "border-warning/40 text-warning"
                  : "border-white/15 text-text-dim",
              ].join(" ")}
            >
              {snapshot.origin === SaveOrigin.ActiveDirectory ? "Active" : "Vault"}
            </span>
            <IntegrityBadge integrity={snapshot.integrity} loading={verifying} />
          </div>
        </div>
      </button>
      {editing ? (
        <textarea
          value={noteDraft}
          onChange={(e) => setNoteDraft(e.target.value)}
          rows={2}
          className="ui-input mt-2 w-full px-2 py-1"
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              saveAnnotation();
            }
          }}
          onBlur={saveAnnotation}
        />
      ) : (
        <p
          className={["mt-2 font-mono text-xs text-text-dim", expanded ? "" : "line-clamp-2"].join(" ")}
          onDoubleClick={() => setEditing(true)}
        >
          {snapshot.note ?? "No notes — double-click to edit"}
        </p>
      )}
      {!editing && (snapshot.note ?? "").length > 80 ? (
        <button type="button" className="ui-focus-ring font-mono text-xs text-accent" onClick={() => setExpanded((v) => !v)}>
          {expanded ? "Less" : "More"}
        </button>
      ) : null}
      <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
        <span className="font-mono text-xs text-text-dim">
          {snapshot.fileCount} files · {formatBytes(snapshot.metadata.byteSize)}
        </span>
        <button
          type="button"
          onClick={onVerify}
          disabled={verifying}
          className="ui-btn border-white/15 px-2 py-0.5 disabled:opacity-40"
        >
          Verify
        </button>
      </div>
    </div>
  );
}

/**
 * @param {{ snapshotId: string, gameId: string, color: string, onAnnotation: (id: string, gameId: string, patch: object) => void }} props
 */
function LabelColorChooser({ snapshotId, gameId, color, onAnnotation }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef(/** @type {HTMLDivElement | null} */ (null));
  const defaultColor = labelColorForId(snapshotId);
  const isDefault = color === defaultColor;

  useEffect(() => {
    if (!open) return;
    const handlePointer = (e) => {
      if (rootRef.current && !rootRef.current.contains(/** @type {Node} */ (e.target))) {
        setOpen(false);
      }
    };
    const handleKey = (e) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", handlePointer);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointer);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative mb-3">
      <p className="text-text-dim">Label colour</p>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="ui-btn mt-1 flex items-center gap-2 border-white/15 px-2 py-1.5 hover:border-white/30"
        aria-expanded={open}
        aria-haspopup="listbox"
        title="Choose label colour"
      >
        <span
          className="h-5 w-5 shrink-0 rounded-full border-2 border-white/30"
          style={{ background: color }}
        />
        <span className="font-mono text-xs text-text-dim">
          {isDefault ? "Default" : "Custom"}
        </span>
      </button>
      {open ? (
        <div
          role="listbox"
          aria-label="Label colours"
          className="absolute left-0 top-full z-20 mt-1 rounded-lg border border-white/15 bg-bg-panel p-2 shadow-elevation"
        >
          <div className="grid grid-cols-3 gap-2">
            {LABEL_COLORS.map((c) => (
              <button
                key={c}
                type="button"
                role="option"
                aria-selected={color === c}
                title={c}
                onClick={() => {
                  if (color !== c) {
                    onAnnotation(snapshotId, gameId, { labelColor: c });
                  }
                  setOpen(false);
                }}
                className={[
                  "h-8 w-8 rounded-full border-2 transition-colors duration-150 hover:border-accent/50",
                  color === c ? "border-accent" : "border-white/20",
                ].join(" ")}
                style={{ background: c }}
              />
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function DetailPanel({
  snapshot,
  game,
  verifying,
  onVerify,
  onRestore,
  onDelete,
  canRollback,
  onRollback,
  rollingBack,
  onAnnotation,
}) {
  if (!snapshot || !game) {
    return (
      <aside className="flex w-72 shrink-0 flex-col border-l border-white/10 bg-bg-panel density-pad">
        <p className="font-mono text-sm text-text-dim">Select a snapshot</p>
      </aside>
    );
  }
  return (
    <aside className="flex h-full min-h-0 w-72 shrink-0 flex-col overflow-hidden border-l border-white/10 bg-bg-panel">
      <div className="density-pad shrink-0 border-b border-white/10">
        <h3 className=" font-display text-lg font-semibold">Details</h3>
        <div className="mt-2">
          <IntegrityBadge integrity={snapshot.integrity} loading={verifying} />
        </div>
      </div>
      <div className="scroll-y-panel min-h-0 flex-1 density-pad font-mono text-xs">
        <LabelColorChooser
          snapshotId={snapshot.id}
          gameId={snapshot.gameId}
          color={snapshot.labelColor}
          onAnnotation={onAnnotation}
        />
        <p className="text-text-dim">Notes</p>
        <p className="mb-3">{snapshot.note ?? "—"}</p>
        <p className="text-text-dim">SHA-256</p>
        <p className="mb-3 break-all">{snapshot.metadata.sha256 ?? "—"}</p>
        <p className="text-text-dim">Path</p>
        <p className="mb-3 break-all">{snapshot.absolutePath}</p>
        <p className="text-text-dim">Files</p>
        <ul className="mb-4 list-disc pl-4">
          {snapshot.files.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={onVerify}
            disabled={verifying}
            className="ui-btn flex w-full items-center justify-center gap-1 border-accent/40 py-1.5 text-accent disabled:opacity-50"
          >
            <ShieldCheck size={16} /> Verify
          </button>
          <button
            type="button"
            onClick={onRestore}
            className="ui-btn w-full border-warning/50 py-1.5 text-warning"
          >
            Restore to Active
          </button>
          <button
            type="button"
            onClick={onRollback}
            disabled={!canRollback || rollingBack}
            className={[
              "ui-btn w-full py-1.5 disabled:opacity-40",
              canRollback ? "border-warning text-warning" : "border-white/15 text-text-dim",
            ].join(" ")}
          >
            Rollback last swap
          </button>
          <button
            type="button"
            onClick={onDelete}
            className="ui-btn w-full border-danger/50 py-1.5 text-danger"
          >
            Delete snapshot
          </button>
        </div>
      </div>
    </aside>
  );
}

function ModalBackdrop({ children, onClose }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6">
      <button type="button" className="absolute inset-0" aria-label="Close" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-white/15 bg-bg-panel p-5 shadow-elevation">
        {children}
      </div>
    </div>
  );
}

function AddGameModal({ open, onClose, onSubmit, loading }) {
  const { pushToast } = useToast();
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [browsing, setBrowsing] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [discovered, setDiscovered] = useState(/** @type {DiscoveredSaveFile[]} */ ([]));

  const refreshScan = useCallback(async (dirPath) => {
    const trimmed = dirPath.trim();
    if (!trimmed) {
      setDiscovered([]);
      return;
    }
    setScanning(true);
    try {
      const result = await scanSaveDirectory(trimmed);
      if (!result.ok) {
        setDiscovered([]);
        pushToast({ type: "error", message: result.error });
        return;
      }
      setDiscovered(result.files);
    } catch (err) {
      setDiscovered([]);
      const message = err instanceof Error ? err.message : "Could not scan save folder.";
      pushToast({ type: "error", message });
    } finally {
      setScanning(false);
    }
  }, [pushToast]);

  useEffect(() => {
    if (!open) return;
    const trimmed = path.trim();
    if (!trimmed) {
      setDiscovered([]);
      return;
    }
    const timer = window.setTimeout(() => {
      refreshScan(trimmed);
    }, 400);
    return () => window.clearTimeout(timer);
  }, [open, path, refreshScan]);

  const handleBrowse = async () => {
    setBrowsing(true);
    try {
      const picked = await pickSaveDirectory();
      if (!picked) return;
      setPath(picked);
      if (!name.trim()) {
        const base = picked.split(/[/\\]/).filter(Boolean).pop();
        if (base) setName(base);
      }
      await refreshScan(picked);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Could not open folder picker.";
      pushToast({ type: "error", message });
    } finally {
      setBrowsing(false);
    }
  };

  if (!open) return null;

  return (
    <ModalBackdrop onClose={onClose}>
      <h3 className="font-display text-lg font-semibold text-accent">Add game</h3>
      <label className="mt-4 block font-mono text-xs text-text-dim">
        Name
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="ui-input mt-1 w-full px-2 py-2"
        />
      </label>
      <label className="mt-3 block font-mono text-xs text-text-dim">
        Save path
        <div className="mt-1 flex gap-2">
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            className="ui-input min-w-0 flex-1 px-2 py-2"
          />
          <button
            type="button"
            disabled={browsing || loading}
            onClick={handleBrowse}
            className="ui-btn shrink-0 border-white/15 px-2 disabled:opacity-50"
          >
            {browsing ? "…" : "Browse"}
          </button>
        </div>
      </label>
      {path.trim() ? (
        <div className="mt-3 max-h-36 overflow-y-auto rounded-lg border border-white/10 bg-bg-primary p-2">
          <p className="font-mono text-xs text-text-dim">
            {scanning ? "Scanning for save files…" : `Found ${discovered.length} save file(s)`}
          </p>
          {discovered.length > 0 ? (
            <ul className="mt-2 space-y-1 font-mono text-xs text-text-primary">
              {discovered.map((f) => (
                <li key={f.absolutePath} className="truncate" title={f.absolutePath}>
                  {f.relativePath} · {formatBytes(f.size)}
                </li>
              ))}
            </ul>
          ) : !scanning ? (
            <p className="mt-1 font-mono text-xs text-warning">
              No .sav, .save, .dat, .bak, .profile, or .json files found in this folder (searched up to 4 levels deep).
            </p>
          ) : null}
        </div>
      ) : null}
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" onClick={onClose} className="ui-btn border-white/15 px-3 py-1">
          Cancel
        </button>
        <button
          type="button"
          disabled={loading || !name.trim() || !path.trim()}
          onClick={() =>
            onSubmit({
              name: name.trim(),
              activeSaveDir: path.trim(),
              discoveredFiles: discovered,
            })
          }
          className="ui-btn border-accent/50 px-3 py-1 text-accent disabled:opacity-40"
        >
          {loading ? "Adding…" : "Confirm"}
        </button>
      </div>
    </ModalBackdrop>
  );
}

function BackupModal({ open, onClose, onConfirm, loading, progress }) {
  const [label, setLabel] = useState("");
  const [note, setNote] = useState("");

  if (!open) return null;

  return (
    <ModalBackdrop onClose={onClose}>
      <h3 className="font-display text-lg font-semibold text-accent">Backup now</h3>
      <label className="mt-4 block font-mono text-xs text-text-dim">
        Label (optional)
        <input
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          className="ui-input mt-1 w-full px-2 py-2"
        />
      </label>
      <label className="mt-3 block font-mono text-xs text-text-dim">
        Notes (optional)
        <textarea
          value={note}
          onChange={(e) => setNote(e.target.value)}
          rows={3}
          className="ui-input mt-1 w-full px-2 py-2"
        />
      </label>
      {loading ? (
        <div className="mt-4">
          <ProgressBar progress={progress} />
        </div>
      ) : null}
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" onClick={onClose} disabled={loading} className="ui-btn border-white/15 px-3 py-1">
          Cancel
        </button>
        <button
          type="button"
          disabled={loading}
          onClick={() => onConfirm({ label: label.trim() || null, note: note.trim() || null })}
          className="ui-btn border-accent/50 px-3 py-1 text-accent disabled:opacity-40"
        >
          {loading ? "Backing up…" : "Confirm backup"}
        </button>
      </div>
    </ModalBackdrop>
  );
}

function RestoreConfirmModal({ open, onClose, onConfirm, game, snapshot, loading, progress }) {
  const [serverWarning, setServerWarning] = useState(/** @type {string | null} */ (null));
  const [warningError, setWarningError] = useState(/** @type {string | null} */ (null));

  useEffect(() => {
    if (!open || !snapshot?.id) {
      setServerWarning(null);
      setWarningError(null);
      return;
    }
    let cancelled = false;
    (async () => {
      const res = await slotforgeApi.destructiveRestoreWarning({ snapshotId: snapshot.id });
      if (cancelled) return;
      if (res.ok) {
        setServerWarning(res.data);
        setWarningError(null);
      } else {
        setServerWarning(null);
        setWarningError(res.error.message);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, snapshot?.id]);

  if (!open || !snapshot || !game) return null;

  return (
    <ModalBackdrop onClose={onClose}>
      <div className="modal-panel">
        <h3 className="font-display text-lg font-semibold text-danger">Restore to active?</h3>
        <p className="mt-2 font-mono text-xs text-text-dim">
          This will overwrite active save files with vault snapshot <strong>{snapshot.label ?? snapshot.fileName}</strong>.
          This action cannot be undone except via rollback.
        </p>
        {serverWarning ? (
          <p className="mt-2 rounded-lg border border-danger/30 bg-danger/5 p-2 font-mono text-xs text-danger">
            {serverWarning}
          </p>
        ) : null}
        {warningError ? (
          <p className="mt-2 font-mono text-xs text-warning">Could not load restore details: {warningError}</p>
        ) : null}
        {game.hasConflict && game.conflictFiles.length > 0 ? (
          <div className="mt-4 max-h-48 overflow-auto rounded-lg border border-warning/30">
            <table className="w-full font-mono text-xs">
              <thead className="text-left text-text-dim">
                <tr>
                  <th className="p-2">File</th>
                  <th className="p-2">Freshness</th>
                </tr>
              </thead>
              <tbody>
                {game.conflictFiles.map((row) => (
                  <tr key={row.path} className="border-t border-white/5">
                    <td className="p-2">{row.path}</td>
                    <td className="p-2 text-warning">{row.freshness}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {game.conflictFiles.map((row) => (
              <pre key={`${row.path}-diff`} className="border-t border-white/5 p-2 font-mono text-xs text-text-dim">
                {row.activeSnippet}
                {"\n---\n"}
                {row.snapshotSnippet}
              </pre>
            ))}
          </div>
        ) : null}
        {loading ? (
          <div className="mt-4">
            <ProgressBar progress={progress} />
          </div>
        ) : null}
        <div className="mt-5 flex justify-end gap-2">
          <button type="button" onClick={onClose} disabled={loading} className="ui-btn border-white/15 px-3 py-1">
            Cancel
          </button>
          <button
            type="button"
            disabled={loading}
            onClick={onConfirm}
            className="ui-btn border-danger/50 px-3 py-1 text-danger disabled:opacity-40"
          >
            {loading ? "Restoring…" : "Confirm restore"}
          </button>
        </div>
      </div>
    </ModalBackdrop>
  );
}

function DeleteConfirmModal({ open, snapshot, onClose, onConfirm, loading }) {
  if (!open || !snapshot) return null;
  return (
    <ModalBackdrop onClose={onClose}>
      <h3 className="font-display text-lg font-semibold text-danger">Delete snapshot?</h3>
      <p className="mt-2 font-mono text-xs text-text-dim">
        Permanently remove <strong>{snapshot.label ?? snapshot.fileName}</strong> from the vault.
      </p>
      <div className="mt-5 flex justify-end gap-2">
        <button type="button" onClick={onClose} className="ui-btn border-white/15 px-3 py-1">
          Cancel
        </button>
        <button
          type="button"
          disabled={loading}
          onClick={onConfirm}
          className="ui-btn border-danger/50 px-3 py-1 text-danger disabled:opacity-40"
        >
          {loading ? "Deleting…" : "Delete"}
        </button>
      </div>
    </ModalBackdrop>
  );
}

function IgnoredGamesModal({ open, onClose }) {
  /** @type {[IgnoredEntry[], function]} */
  const [entries, setEntries] = useState([]);
  const [loading, setLoading] = useState(false);
  const [adding, setAdding] = useState(false);
  const [newPath, setNewPath] = useState("");
  const [newName, setNewName] = useState("");
  const { pushToast } = useToast();

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const res = await slotforgeApi.listIgnoredGames();
      if (!res.ok) {
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      setEntries(res.data.entries ?? []);
    } catch (err) {
      pushToast({ type: "error", message: unexpectedError(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    if (!open) return;
    refresh();
  }, [open, refresh]);

  const handlePickFolder = async () => {
    try {
      const picked = await pickSaveDirectory();
      if (picked) setNewPath(picked);
    } catch (err) {
      pushToast({ type: "error", message: unexpectedError(err) });
    }
  };

  const handleAdd = async () => {
    const path = newPath.trim();
    if (!path) {
      pushToast({ type: "error", message: "Choose a folder path to ignore." });
      return;
    }
    setAdding(true);
    try {
      const res = await slotforgeApi.addIgnoredPath({
        path,
        name: newName.trim() || null,
      });
      if (!res.ok) {
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      setEntries(res.data.entries ?? []);
      setNewPath("");
      setNewName("");
      pushToast({ type: "success", message: "Path added to ignored list." });
    } catch (err) {
      pushToast({ type: "error", message: unexpectedError(err) });
    } finally {
      setAdding(false);
    }
  };

  const handleRemove = async (path) => {
    try {
      const res = await slotforgeApi.removeIgnoredPath({ path });
      if (!res.ok) {
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      setEntries(res.data.entries ?? []);
      pushToast({ type: "success", message: "Removed from ignored list. Rescan to show games again." });
    } catch (err) {
      pushToast({ type: "error", message: unexpectedError(err) });
    }
  };

  if (!open) return null;

  return (
    <ModalBackdrop onClose={onClose}>
      <div className="flex max-h-[min(80vh,32rem)] flex-col">
        <h3 className=" font-display text-lg font-semibold text-accent">Ignored games</h3>
        <p className="mt-2 font-mono text-xs text-text-dim">
          Games and folders listed here are excluded from scans. Your save files on disk are never deleted.
        </p>
        <div className="mt-4 min-h-0 flex-1 overflow-y-auto rounded-lg border border-white/10">
          {loading ? (
            <p className="p-4 font-mono text-xs text-text-dim">Loading…</p>
          ) : entries.length === 0 ? (
            <p className="p-4 font-mono text-xs text-text-dim">No ignored games or folders yet.</p>
          ) : (
            <ul className="divide-y divide-white/10">
              {entries.map((entry) => (
                <li key={entry.path} className="flex items-start gap-2 p-3">
                  <div className="min-w-0 flex-1">
                    <div className="truncate font-display text-sm font-semibold">
                      {entry.name ?? entry.path.split(/[/\\]/).pop() ?? entry.path}
                    </div>
                    <div className="break-all font-mono text-xs text-text-dim">{entry.path}</div>
                  </div>
                  <button
                    type="button"
                    onClick={() => handleRemove(entry.path)}
                    className="ui-btn shrink-0 border-white/15 px-2 py-1 text-text-dim hover:border-danger/40 hover:text-danger"
                  >
                    Remove
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="mt-4 shrink-0 border-t border-white/10 pt-4">
          <p className="mb-2 font-mono text-xs text-text-dim">Add folder or game save directory</p>
          <label className="mb-2 block font-mono text-xs text-text-dim">
            Display name (optional)
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="e.g. Old RPG saves"
              className="ui-input mt-1 w-full px-2 py-2"
            />
          </label>
          <label className="mb-2 block font-mono text-xs text-text-dim">
            Path
            <div className="mt-1 flex gap-2">
              <input
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
                placeholder="C:\Users\…\Saved Games\…"
                className="ui-input min-w-0 flex-1 px-2 py-2"
              />
              <button
                type="button"
                onClick={handlePickFolder}
                className="ui-btn flex shrink-0 items-center gap-1 border-white/15 px-2 py-2"
              >
                <FolderPlus size={16} /> Browse
              </button>
            </div>
          </label>
          <button
            type="button"
            disabled={adding || !newPath.trim()}
            onClick={handleAdd}
            className="ui-btn border-accent/40 px-3 py-1 text-accent disabled:opacity-40"
          >
            {adding ? "Adding…" : "Add to ignore list"}
          </button>
        </div>
        <div className="mt-4 flex justify-end">
          <button type="button" onClick={onClose} className="ui-btn border-white/15 px-3 py-1">
            Close
          </button>
        </div>
      </div>
    </ModalBackdrop>
  );
}

function ThemeEditor({ onClose, onOpenIgnoredGames, onImportError, onImportSuccess }) {
  const {
    theme,
    applyPreset,
    setToken,
    setDensity,
    setFontSize,
    toggleScanlines,
    toggleGlow,
    exportTheme,
    importTheme,
  } = useTheme();
  const fileRef = useRef(/** @type {HTMLInputElement | null} */ (null));

  const handleImport = (e) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const result = importTheme(String(reader.result ?? ""));
      if (!result.ok) onImportError(result.error);
      else onImportSuccess();
    };
    reader.readAsText(file);
  };

  return (
    <div className="absolute inset-0 z-40 flex bg-black/60">
      <div className="m-auto flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden rounded-lg border border-white/15 bg-bg-panel shadow-elevation">
        <div className="flex items-center justify-between border-b border-white/10 density-pad">
          <h2 className="font-display text-xl font-semibold">Settings</h2>
          <button type="button" onClick={onClose} aria-label="Close settings" className="ui-icon-btn">
            <X size={20} />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto density-pad font-mono text-xs">
          <p className="mb-2 text-text-dim">Library</p>
          <button
            type="button"
            onClick={onOpenIgnoredGames}
            className="ui-btn mb-4 flex w-full items-center justify-center gap-2 border-white/15 py-2 text-text-primary hover:border-accent/40"
          >
            <EyeOff size={16} className="text-accent" />
            Ignored games
          </button>
          <p className="mb-2 text-text-dim">Theme · Presets</p>
          <div className="mb-4 flex flex-wrap gap-2">
            {Object.values(THEME_PRESETS).map((preset) => (
              <button
                key={preset.name}
                type="button"
                onClick={() => applyPreset(preset)}
                className={[
                  "ui-btn px-2 py-1",
                  theme.presetName === preset.name ? "border-accent text-accent" : "border-white/15",
                ].join(" ")}
              >
                {preset.name}
              </button>
            ))}
            <button type="button" onClick={() => applyPreset(THEME_PRESET_DARKROOM)} className="ui-btn border-white/15 px-2 py-1">
              Reset DARKROOM
            </button>
          </div>
          {(
            [
              ["accent", "Accent"],
              ["bgPrimary", "Background"],
              ["bgPanel", "Panel"],
              ["textPrimary", "Text"],
              ["textDim", "Dim text"],
              ["danger", "Danger"],
              ["warning", "Warning"],
            ]
          ).map(([key, label]) => (
            <label key={key} className="mb-2 flex items-center justify-between gap-4">
              <span className="text-text-dim">{label}</span>
              <input
                type="color"
                value={theme.tokens[key]}
                onChange={(e) => setToken(key, e.target.value)}
              />
            </label>
          ))}
          <label className="mb-3 mt-3 block text-text-dim">
            Font size ({theme.fontSize}px)
            <input
              type="range"
              min={12}
              max={22}
              value={theme.fontSize}
              onChange={(e) => setFontSize(Number(e.target.value))}
              className="mt-1 w-full"
            />
          </label>
          <div className="mb-3 flex flex-wrap gap-4">
            <label className="flex items-center gap-2">
              <input
                type="radio"
                checked={theme.density === "comfortable"}
                onChange={() => setDensity("comfortable")}
              />
              Comfortable
            </label>
            <label className="flex items-center gap-2">
              <input type="radio" checked={theme.density === "compact"} onChange={() => setDensity("compact")} />
              Compact
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={theme.scanlinesEnabled} onChange={toggleScanlines} />
              Scanlines
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={theme.glowEnabled} onChange={toggleGlow} />
              Highlight borders
            </label>
          </div>
          <div
            className="ui-panel-interactive p-4"
            style={{ background: theme.tokens.bgPrimary, color: theme.tokens.textPrimary }}
          >
            <p className="font-display text-lg" style={{ color: theme.tokens.accent }}>
              Live preview
            </p>
            <p style={{ color: theme.tokens.textDim }}>Panel density: {theme.density}</p>
          </div>
          <div className="mt-4 flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => {
                const blob = new Blob([exportTheme()], { type: "application/json" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = "slotforge-theme.json";
                a.click();
                URL.revokeObjectURL(url);
              }}
              className="ui-btn border-accent/40 px-3 py-1 text-accent"
            >
              Export JSON
            </button>
            <button
              type="button"
              onClick={() => fileRef.current?.click()}
              className="ui-btn border-white/15 px-3 py-1"
            >
              Import JSON
            </button>
            <input ref={fileRef} type="file" accept="application/json" className="hidden" onChange={handleImport} />
          </div>
        </div>
      </div>
    </div>
  );
}

function PanelCollapseButton({ collapsed, onClick, side }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "ui-icon-btn absolute top-1/2 z-20 -translate-y-1/2 bg-bg-panel",
        side === "left" ? (collapsed ? "left-0" : "-left-3") : collapsed ? "right-0" : "-right-3",
      ].join(" ")}
      aria-label={collapsed ? "Expand panel" : "Collapse panel"}
    >
      {side === "left" ? (
        collapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />
      ) : collapsed ? (
        <ChevronLeft size={16} />
      ) : (
        <ChevronRight size={16} />
      )}
    </button>
  );
}

// ============================================================================
// APP
// ============================================================================

function SlotForgeAppInner() {
  const { theme } = useTheme();
  const { pushToast } = useToast();
  const initialDb = useMemo(
    () => ({
      games: [],
      vaultByGameId: {},
      lastSwap: null,
    }),
    []
  );

  const [state, dispatch] = useReducer(appReducer, initialDb, makeInitialAppState);
  const [gameQuery, setGameQuery] = useState("");
  const [vaultSort, setVaultSort] = useState("date-desc");
  const [labelFilter, setLabelFilter] = useState("");
  const [integrityFilter, setIntegrityFilter] = useState("all");
  const [colorFilter, setColorFilter] = useState("all");
  const [rescanningActive, setRescanningActive] = useState(false);
  const [ignoredGamesOpen, setIgnoredGamesOpen] = useState(false);
  const [gameContextMenu, setGameContextMenu] = useState(
    /** @type {{ gameId: string, gameName: string, x: number, y: number } | null} */ (null)
  );
  const [removingGame, setRemovingGame] = useState(false);

  const selectedGame = useMemo(
    () => state.games.find((g) => g.id === state.selectedGameId) ?? null,
    [state.games, state.selectedGameId]
  );
  const snapshots = useMemo(() => {
    if (!state.selectedGameId) return [];
    return state.vaultByGameId[state.selectedGameId] ?? [];
  }, [state.selectedGameId, state.vaultByGameId]);
  const selectedSnapshot = useMemo(() => {
    if (!state.selectedSnapshotId) return null;
    return snapshots.find((s) => s.id === state.selectedSnapshotId) ?? null;
  }, [snapshots, state.selectedSnapshotId]);

  const totalVaultBytes = useMemo(() => {
    let sum = 0;
    for (const list of Object.values(state.vaultByGameId)) {
      for (const snap of list) sum += snap.metadata.byteSize;
    }
    return sum;
  }, [state.vaultByGameId]);

  const progressPct = useMemo(() => {
    const { current, total } = state.operations.progress;
    if (!total) return state.ui.loading.backingUp || state.ui.loading.restoring ? 45 : 0;
    return Math.round((current / total) * 100);
  }, [state.operations.progress, state.ui.loading]);

  const logOp = useCallback((type, result, message, gameId = null, snapshotId = null) => {
    /** @type {OperationLog} */
    const entry = {
      id: `op-${Date.now()}`,
      type,
      result,
      message,
      timestamp: new Date().toISOString(),
      gameId,
      snapshotId,
    };
    dispatch({ type: "SET_LAST_OP", payload: entry });
    return entry;
  }, []);

  const applyLibrary = useCallback((library) => {
    dispatch({
      type: "SET_DB",
      payload: {
        games: library.games,
        vaultByGameId: library.vaultByGameId,
        lastSwap: library.lastSwap ?? null,
      },
    });
  }, []);

  useSlotForgeBoot(applyLibrary, pushToast, dispatch);

  const {
    handleScan,
    handleRescanActive,
    handleAddGame,
    handleBackup,
    handleRestore,
    handleRollback,
    handleVerifySnapshot,
    handleVerifyAll,
    handleAnnotation,
    handleDelete,
    handleRemoveGame,
  } = useSlotForgeActions({
    dispatch,
    selectedGameId: state.selectedGameId,
    vaultByGameId: state.vaultByGameId,
    lastSwap: state.operations.lastSwap,
    selectedGame,
    selectedSnapshot,
    applyLibrary,
    logOp,
    pushToast,
    setRescanningActive,
    setGameContextMenu,
    setRemovingGame,
  });

  const sidebarCollapsed = state.ui.panels.sidebarCollapsed;
  const detailCollapsed = state.ui.panels.detailCollapsed;
  const canRollback = Boolean(state.operations.lastSwap);

  return (
    <AppShell
      scanlines={theme.scanlinesEnabled}
      sidebar={
        sidebarCollapsed ? (
          <div className="relative w-0 shrink-0">
            <PanelCollapseButton
              collapsed
              side="left"
              onClick={() => dispatch({ type: "TOGGLE_PANEL", payload: "sidebar" })}
            />
          </div>
        ) : (
          <div className="relative flex h-full min-h-0 shrink-0">
            <GameSidebar
              games={state.games}
              query={gameQuery}
              onQuery={setGameQuery}
              selectedId={state.selectedGameId}
              onSelect={(id) => dispatch({ type: "SELECT_GAME", payload: id })}
              onScan={handleScan}
              scanning={state.ui.loading.scanning}
              onAdd={() => dispatch({ type: "OPEN_MODAL", payload: "addGame" })}
              onSettings={() => dispatch({ type: "SET_SETTINGS_OPEN", payload: true })}
              onGameContextMenu={(game, e) => {
                setGameContextMenu({
                  gameId: game.id,
                  gameName: game.name,
                  x: e.clientX,
                  y: e.clientY,
                });
              }}
            />
            <PanelCollapseButton
              collapsed={false}
              side="left"
              onClick={() => dispatch({ type: "TOGGLE_PANEL", payload: "sidebar" })}
            />
          </div>
        )
      }
      main={
        <VaultBrowser
          game={selectedGame}
          snapshots={snapshots}
          sort={vaultSort}
          onSort={setVaultSort}
          labelFilter={labelFilter}
          onLabelFilter={setLabelFilter}
          integrityFilter={integrityFilter}
          onIntegrityFilter={setIntegrityFilter}
          colorFilter={colorFilter}
          onColorFilter={setColorFilter}
          selectedSnapshotId={state.selectedSnapshotId}
          onSelectSnapshot={(id) => dispatch({ type: "SELECT_SNAPSHOT", payload: id })}
          onBackup={() => dispatch({ type: "OPEN_MODAL", payload: "backup" })}
          onVerifyAll={handleVerifyAll}
          batchVerifying={state.ui.loading.batchVerifying}
          onVerifySnapshot={handleVerifySnapshot}
          onAnnotation={handleAnnotation}
          verifyingId={state.ui.loading.verifying ? state.selectedSnapshotId : null}
          onRescanActive={handleRescanActive}
          rescanningActive={rescanningActive}
        />
      }
      detail={
        detailCollapsed ? (
          <div className="relative w-0 shrink-0">
            <PanelCollapseButton
              collapsed
              side="right"
              onClick={() => dispatch({ type: "TOGGLE_PANEL", payload: "detail" })}
            />
          </div>
        ) : (
          <div className="relative flex h-full min-h-0 shrink-0">
            <DetailPanel
              snapshot={selectedSnapshot}
              game={selectedGame}
              verifying={state.ui.loading.verifying}
              onVerify={() => selectedSnapshot && handleVerifySnapshot(selectedSnapshot.id)}
              onRestore={() => dispatch({ type: "OPEN_MODAL", payload: "restore" })}
              onDelete={() => dispatch({ type: "OPEN_MODAL", payload: "delete" })}
              canRollback={canRollback}
              onRollback={handleRollback}
              rollingBack={state.ui.loading.rollingBack}
              onAnnotation={handleAnnotation}
            />
            <PanelCollapseButton
              collapsed={false}
              side="right"
              onClick={() => dispatch({ type: "TOGGLE_PANEL", payload: "detail" })}
            />
          </div>
        )
      }
      statusBar={
        <StatusBar
          lastOp={state.operations.lastOp}
          totalVaultBytes={totalVaultBytes}
          activeGameName={selectedGame?.name}
        />
      }
      overlay={
        <>
          {state.ui.loading.batchVerifying || state.ui.loading.backingUp ? (
            <div className="pointer-events-none absolute bottom-14 left-1/2 z-30 w-64 -translate-x-1/2">
              <ProgressBar progress={progressPct} />
            </div>
          ) : null}
          <AddGameModal
            open={state.ui.modals.addGameOpen}
            onClose={() => dispatch({ type: "CLOSE_MODAL", payload: "addGame" })}
            onSubmit={handleAddGame}
            loading={state.ui.loading.addingGame}
          />
          <BackupModal
            open={state.ui.modals.backupOpen}
            onClose={() => dispatch({ type: "CLOSE_MODAL", payload: "backup" })}
            onConfirm={handleBackup}
            loading={state.ui.loading.backingUp}
            progress={progressPct}
          />
          <RestoreConfirmModal
            open={state.ui.modals.restoreOpen}
            onClose={() => dispatch({ type: "CLOSE_MODAL", payload: "restore" })}
            onConfirm={handleRestore}
            game={selectedGame}
            snapshot={selectedSnapshot}
            loading={state.ui.loading.restoring}
            progress={progressPct}
          />
          <DeleteConfirmModal
            open={state.ui.modals.deleteOpen}
            snapshot={selectedSnapshot}
            onClose={() => dispatch({ type: "CLOSE_MODAL", payload: "delete" })}
            onConfirm={handleDelete}
            loading={state.ui.loading.deleting}
          />
          {gameContextMenu ? (
            <GameContextMenu
              x={gameContextMenu.x}
              y={gameContextMenu.y}
              gameName={gameContextMenu.gameName}
              onClose={() => setGameContextMenu(null)}
              onDelete={() => {
                if (!removingGame) handleRemoveGame(gameContextMenu.gameId);
              }}
            />
          ) : null}
          <IgnoredGamesModal open={ignoredGamesOpen} onClose={() => setIgnoredGamesOpen(false)} />
          {state.settingsViewOpen ? (
            <ThemeEditor
              onClose={() => dispatch({ type: "SET_SETTINGS_OPEN", payload: false })}
              onOpenIgnoredGames={() => {
                dispatch({ type: "SET_SETTINGS_OPEN", payload: false });
                setIgnoredGamesOpen(true);
              }}
              onImportError={(msg) => pushToast({ type: "error", message: msg })}
              onImportSuccess={() => pushToast({ type: "success", message: "Theme imported." })}
            />
          ) : null}
        </>
      }
    />
  );
}

export default function SlotForgeApp() {
  return (
    <ThemeProvider>
      <ToastProvider>
        <SlotForgeAppInner />
      </ToastProvider>
    </ThemeProvider>
  );
}
