import { useEffect } from "react";
import { slotforgeApi, unexpectedError } from "../api/slotforgeApi.js";

const BOOT_SCAN_DELAY_MS = 1000;

/**
 * Loads cached library on mount, then runs a background scan after a short delay.
 *
 * @param {(library: { games: unknown[], vaultByGameId: Record<string, unknown[]>, lastSwap?: unknown }) => void} applyLibrary
 * @param {(toast: { type: string, message: string }) => void} pushToast
 * @param {import("react").Dispatch<unknown>} dispatch
 */
export function useSlotForgeBoot(applyLibrary, pushToast, dispatch) {
  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const res = await slotforgeApi.loadLibrary();
        if (cancelled) return;
        if (!res.ok) {
          pushToast({ type: "error", message: res.error.message });
          return;
        }
        applyLibrary(res.data);

        await new Promise((resolve) => setTimeout(resolve, BOOT_SCAN_DELAY_MS));
        if (cancelled) return;

        dispatch({ type: "SET_LOADING", payload: { key: "scanning", value: true } });
        const scanRes = await slotforgeApi.scanGamesBackground();
        if (cancelled) return;
        dispatch({ type: "SET_LOADING", payload: { key: "scanning", value: false } });
        if (!scanRes.ok) {
          pushToast({ type: "error", message: scanRes.error.message });
          return;
        }
        applyLibrary(scanRes.data);
      } catch (err) {
        if (cancelled) return;
        pushToast({ type: "error", message: unexpectedError(err) });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [applyLibrary, pushToast, dispatch]);
}
