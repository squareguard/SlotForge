import { useCallback, useRef } from "react";
import { scanSaveDirectory, slotforgeApi, unexpectedError } from "../api/slotforgeApi.js";
import { ResolutionChoice, SaveOrigin } from "../domainEnums.js";

/**
 * Tauri IPC action handlers for library, vault, and game management.
 *
 * @param {object} params
 * @param {import("react").Dispatch<unknown>} params.dispatch
 * @param {string | null} params.selectedGameId
 * @param {Record<string, object[]>} params.vaultByGameId
 * @param {object | null} params.lastSwap
 * @param {object | null} params.selectedGame
 * @param {object | null} params.selectedSnapshot
 * @param {(library: object) => void} params.applyLibrary
 * @param {(type: string, result: string, message: string, gameId?: string | null, snapshotId?: string | null) => void} params.logOp
 * @param {(toast: { type: string, message: string }) => void} params.pushToast
 * @param {(active: boolean) => void} params.setRescanningActive
 * @param {(menu: object | null) => void} params.setGameContextMenu
 * @param {(removing: boolean) => void} params.setRemovingGame
 */
export function useSlotForgeActions({
  dispatch,
  selectedGameId,
  vaultByGameId,
  lastSwap,
  selectedGame,
  selectedSnapshot,
  applyLibrary,
  logOp,
  pushToast,
  setRescanningActive,
  setGameContextMenu,
  setRemovingGame,
}) {
  const colorSaveTimersRef = useRef(new Map());
  const annotationChainRef = useRef(Promise.resolve());

  const handleScan = useCallback(async () => {
    dispatch({ type: "SET_LOADING", payload: { key: "scanning", value: true } });
    try {
      const res = await slotforgeApi.scanGamesBackground();
      if (!res.ok) {
        logOp("scan", "failure", res.error.message);
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      applyLibrary(res.data);
      logOp("scan", "success", `Library has ${res.data.games.length} game(s).`);
      pushToast({ type: "success", message: "Scan complete." });
    } catch (err) {
      const message = unexpectedError(err);
      logOp("scan", "failure", message);
      pushToast({ type: "error", message });
    } finally {
      dispatch({ type: "SET_LOADING", payload: { key: "scanning", value: false } });
    }
  }, [applyLibrary, dispatch, logOp, pushToast]);

  const handleRescanActive = useCallback(async () => {
    if (!selectedGame) return;
    setRescanningActive(true);
    try {
      const scan = await scanSaveDirectory(selectedGame.activeSaveDir);
      if (!scan.ok) {
        pushToast({ type: "error", message: scan.error });
        return;
      }
      const res = await slotforgeApi.loadLibrary();
      if (res.ok) {
        applyLibrary(res.data);
        const list = res.data.vaultByGameId[selectedGame.id] ?? [];
        const firstActive = list.find((s) => s.origin === SaveOrigin.ActiveDirectory);
        if (firstActive) {
          dispatch({ type: "SELECT_SNAPSHOT", payload: firstActive.id });
        }
      } else {
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      pushToast({
        type: scan.files.length > 0 ? "success" : "error",
        message:
          scan.files.length > 0
            ? `Found ${scan.files.length} save file(s) in folder.`
            : "No save files found in that folder.",
      });
    } catch (err) {
      pushToast({ type: "error", message: unexpectedError(err) });
    } finally {
      setRescanningActive(false);
    }
  }, [applyLibrary, dispatch, pushToast, selectedGame, setRescanningActive]);

  const handleAddGame = useCallback(
    async ({ name, activeSaveDir, discoveredFiles }) => {
      dispatch({ type: "SET_LOADING", payload: { key: "addingGame", value: true } });
      try {
        let files = Array.isArray(discoveredFiles) ? discoveredFiles : [];
        if (files.length === 0 && activeSaveDir.trim()) {
          const scan = await scanSaveDirectory(activeSaveDir);
          if (!scan.ok) {
            pushToast({ type: "error", message: scan.error });
            return;
          }
          files = scan.files;
        }
        const res = await slotforgeApi.addGame({ name, activeSaveDir });
        dispatch({ type: "CLOSE_MODAL", payload: "addGame" });
        if (!res.ok) {
          logOp("add_game", "failure", res.error.message);
          pushToast({ type: "error", message: res.error.message });
          return;
        }
        applyLibrary(res.data.library);
        dispatch({ type: "SELECT_GAME", payload: res.data.game.id });
        const count = res.data.discoveredCount ?? files.length;
        logOp("add_game", "success", `Added ${res.data.game.name} (${count} save file(s)).`, res.data.game.id);
        pushToast({
          type: count > 0 ? "success" : "error",
          message:
            count > 0
              ? `Added ${res.data.game.name} with ${count} save file(s).`
              : `Added ${res.data.game.name}, but no save files were found in that folder.`,
        });
      } catch (err) {
        const message = unexpectedError(err);
        logOp("add_game", "failure", message);
        pushToast({ type: "error", message });
      } finally {
        dispatch({ type: "SET_LOADING", payload: { key: "addingGame", value: false } });
      }
    },
    [applyLibrary, dispatch, logOp, pushToast]
  );

  const handleBackup = useCallback(
    async ({ label, note }) => {
      if (!selectedGameId) return;
      dispatch({ type: "SET_LOADING", payload: { key: "backingUp", value: true } });
      dispatch({
        type: "SET_PROGRESS",
        payload: { type: "backup", current: 0, total: 100, message: "Backing up…" },
      });
      try {
        const res = await slotforgeApi.backupGame({
          gameId: selectedGameId,
          label,
          note,
        });
        dispatch({ type: "CLOSE_MODAL", payload: "backup" });
        if (!res.ok) {
          logOp("backup", "failure", res.error.message, selectedGameId);
          pushToast({ type: "error", message: res.error.message });
          return;
        }
        applyLibrary(res.data.library);
        dispatch({ type: "SELECT_SNAPSHOT", payload: res.data.snapshot.id });
        logOp("backup", "success", "Backup created.", selectedGameId, res.data.snapshot.id);
        pushToast({ type: "success", message: "Backup created." });
      } catch (err) {
        const message = unexpectedError(err);
        logOp("backup", "failure", message, selectedGameId);
        pushToast({ type: "error", message });
      } finally {
        dispatch({ type: "SET_LOADING", payload: { key: "backingUp", value: false } });
        dispatch({ type: "SET_PROGRESS", payload: { type: null, current: 0, total: 0, message: null } });
      }
    },
    [applyLibrary, dispatch, logOp, pushToast, selectedGameId]
  );

  const handleRestore = useCallback(async () => {
    if (!selectedSnapshot) return;
    dispatch({ type: "SET_LOADING", payload: { key: "restoring", value: true } });
    try {
      const res = await slotforgeApi.restoreSnapshot({
        snapshotId: selectedSnapshot.id,
        confirmedDestructive: true,
        resolutionChoice: ResolutionChoice.KeepSource,
      });
      dispatch({ type: "CLOSE_MODAL", payload: "restore" });
      if (!res.ok) {
        logOp("restore", "failure", res.error.message, selectedSnapshot.gameId, selectedSnapshot.id);
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      applyLibrary(res.data.library);
      dispatch({ type: "SET_LAST_SWAP", payload: res.data.lastSwap });
      logOp("restore", "success", "Restored to active.", selectedSnapshot.gameId, selectedSnapshot.id);
      pushToast({ type: "success", message: "Restore complete. Rollback available." });
    } catch (err) {
      const message = unexpectedError(err);
      logOp("restore", "failure", message, selectedSnapshot.gameId, selectedSnapshot.id);
      pushToast({ type: "error", message });
    } finally {
      dispatch({ type: "SET_LOADING", payload: { key: "restoring", value: false } });
    }
  }, [applyLibrary, dispatch, logOp, pushToast, selectedSnapshot]);

  const handleRollback = useCallback(async () => {
    if (!lastSwap) return;
    dispatch({ type: "SET_LOADING", payload: { key: "rollingBack", value: true } });
    try {
      const res = await slotforgeApi.rollbackSwap();
      if (!res.ok) {
        logOp("rollback", "failure", res.error.message);
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      applyLibrary(res.data);
      dispatch({ type: "SET_LAST_SWAP", payload: null });
      logOp("rollback", "success", "Rollback complete.");
      pushToast({ type: "success", message: "Rollback complete." });
    } catch (err) {
      const message = unexpectedError(err);
      logOp("rollback", "failure", message);
      pushToast({ type: "error", message });
    } finally {
      dispatch({ type: "SET_LOADING", payload: { key: "rollingBack", value: false } });
    }
  }, [applyLibrary, dispatch, lastSwap, logOp, pushToast]);

  const handleVerifySnapshot = useCallback(
    async (snapshotId) => {
      dispatch({ type: "SET_LOADING", payload: { key: "verifying", value: true } });
      try {
        const res = await slotforgeApi.verifySnapshot({ snapshotId });
        if (!res.ok) {
          logOp("verify", "failure", res.error.message, null, snapshotId);
          pushToast({ type: "error", message: res.error.message });
          return;
        }
        applyLibrary(res.data.library);
        logOp("verify", "success", `Integrity: ${res.data.snapshot.integrity}.`, null, snapshotId);
        pushToast({ type: "success", message: `Verified: ${res.data.snapshot.integrity}` });
      } catch (err) {
        const message = unexpectedError(err);
        logOp("verify", "failure", message, null, snapshotId);
        pushToast({ type: "error", message });
      } finally {
        dispatch({ type: "SET_LOADING", payload: { key: "verifying", value: false } });
      }
    },
    [applyLibrary, dispatch, logOp, pushToast]
  );

  const handleVerifyAll = useCallback(async () => {
    if (!selectedGameId) return;
    dispatch({ type: "SET_LOADING", payload: { key: "batchVerifying", value: true } });
    const list = vaultByGameId[selectedGameId] ?? [];
    dispatch({
      type: "SET_PROGRESS",
      payload: { type: "verify_all", current: 0, total: list.length, message: "Verifying…" },
    });
    try {
      const res = await slotforgeApi.verifyAllSnapshots({ gameId: selectedGameId });
      if (!res.ok) {
        logOp("verify_all", "failure", res.error.message, selectedGameId);
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      applyLibrary(res.data.library);
      logOp("verify_all", "success", `Verified ${res.data.verifiedCount} snapshot(s).`, selectedGameId);
      pushToast({ type: "success", message: `Verified ${res.data.verifiedCount} snapshot(s).` });
    } catch (err) {
      const message = unexpectedError(err);
      logOp("verify_all", "failure", message, selectedGameId);
      pushToast({ type: "error", message });
    } finally {
      dispatch({ type: "SET_LOADING", payload: { key: "batchVerifying", value: false } });
      dispatch({ type: "SET_PROGRESS", payload: { type: null, current: 0, total: 0, message: null } });
    }
  }, [applyLibrary, dispatch, logOp, pushToast, selectedGameId, vaultByGameId]);

  const findGameIdForSnapshot = useCallback(
    (snapshotId) => {
      for (const [gameId, list] of Object.entries(vaultByGameId)) {
        if (list.some((s) => s.id === snapshotId)) return gameId;
      }
      return selectedGameId;
    },
    [selectedGameId, vaultByGameId]
  );

  const persistAnnotation = useCallback(
    (snapshotId, patch) => {
      annotationChainRef.current = annotationChainRef.current
        .then(async () => {
          const res = await slotforgeApi.updateAnnotation({ snapshotId, ...patch });
          if (!res.ok) {
            pushToast({ type: "error", message: res.error.message });
            return;
          }
          const isColorOnly =
            patch.labelColor != null && !("label" in patch) && !("note" in patch);
          if (isColorOnly) {
            const snap = res.data.snapshot;
            dispatch({
              type: "PATCH_SNAPSHOT",
              payload: { gameId: snap.gameId, snapshotId: snap.id, patch: snap },
            });
          } else {
            applyLibrary(res.data.library);
            logOp("annotate", "success", "Annotation saved.", null, snapshotId);
          }
        })
        .catch((err) => {
          pushToast({ type: "error", message: unexpectedError(err) });
        });
    },
    [applyLibrary, dispatch, logOp, pushToast]
  );

  const handleAnnotation = useCallback(
    (snapshotId, gameId, patch) => {
      const isColorOnly =
        patch.labelColor != null && !("label" in patch) && !("note" in patch);

      if (isColorOnly) {
        const resolvedGameId = gameId ?? findGameIdForSnapshot(snapshotId);
        if (resolvedGameId) {
          dispatch({
            type: "PATCH_SNAPSHOT",
            payload: {
              gameId: resolvedGameId,
              snapshotId,
              patch: { labelColor: patch.labelColor },
            },
          });
        }
        const existing = colorSaveTimersRef.current.get(snapshotId);
        if (existing) clearTimeout(existing);
        colorSaveTimersRef.current.set(
          snapshotId,
          setTimeout(() => {
            colorSaveTimersRef.current.delete(snapshotId);
            persistAnnotation(snapshotId, patch);
          }, 300)
        );
        return;
      }

      persistAnnotation(snapshotId, patch);
    },
    [dispatch, findGameIdForSnapshot, persistAnnotation]
  );

  const handleDelete = useCallback(async () => {
    if (!selectedSnapshot) return;
    dispatch({ type: "SET_LOADING", payload: { key: "deleting", value: true } });
    try {
      const res = await slotforgeApi.deleteSnapshot({
        snapshotId: selectedSnapshot.id,
        confirmed: true,
      });
      dispatch({ type: "CLOSE_MODAL", payload: "delete" });
      if (!res.ok) {
        logOp("delete", "failure", res.error.message, selectedSnapshot.gameId, selectedSnapshot.id);
        pushToast({ type: "error", message: res.error.message });
        return;
      }
      applyLibrary(res.data);
      logOp("delete", "success", "Snapshot deleted.", selectedSnapshot.gameId, selectedSnapshot.id);
      pushToast({ type: "success", message: "Snapshot deleted." });
    } catch (err) {
      const message = unexpectedError(err);
      logOp("delete", "failure", message, selectedSnapshot.gameId, selectedSnapshot.id);
      pushToast({ type: "error", message });
    } finally {
      dispatch({ type: "SET_LOADING", payload: { key: "deleting", value: false } });
    }
  }, [applyLibrary, dispatch, logOp, pushToast, selectedSnapshot]);

  const handleRemoveGame = useCallback(
    async (gameId) => {
      setGameContextMenu(null);
      setRemovingGame(true);
      try {
        const res = await slotforgeApi.ignoreGameFromLibrary({ gameId });
        if (!res.ok) {
          pushToast({ type: "error", message: res.error.message });
          return;
        }
        applyLibrary(res.data.library);
        const stillSelected = res.data.library.games.some((g) => g.id === selectedGameId);
        if (!stillSelected) {
          dispatch({ type: "SELECT_GAME", payload: res.data.library.games[0]?.id ?? null });
        }
        pushToast({
          type: "success",
          message:
            "Game removed from library and added to ignored list. Files on disk were not deleted.",
        });
      } catch (err) {
        pushToast({ type: "error", message: unexpectedError(err) });
      } finally {
        setRemovingGame(false);
      }
    },
    [applyLibrary, dispatch, pushToast, selectedGameId, setGameContextMenu, setRemovingGame]
  );

  return {
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
  };
}
