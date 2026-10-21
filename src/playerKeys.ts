import { useEffect } from "react";
import { emitTo } from "@tauri-apps/api/event";
import { onPlayerEvent, PLAYER_KEY_EVENT, PLAYER_OPEN_CLASS } from "./api/player";

/** The player controls window's label (`CONTROLS_WINDOW` in the backend). */
const CONTROLS_WINDOW = "player-controls";

/**
 * While a video plays, keys pressed in the main window go to the player controls, so the
 * shortcuts work whichever Spaceport window has focus. Typing in a text field is left alone.
 */
export function useForwardPlayerKeys() {
  useEffect(() => {
    let playing = false;
    const unlisten = onPlayerEvent((event) => {
      if (event.kind === "loaded" || event.kind === "fallback") playing = true;
      if (event.kind === "closed" || event.kind === "failed") playing = false;
    });
    const onKey = (e: KeyboardEvent) => {
      if (!playing || e.ctrlKey || e.altKey || e.metaKey) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      e.preventDefault();
      emitTo(CONTROLS_WINDOW, PLAYER_KEY_EVENT, e.key).catch(() => {});
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      unlisten.then((stop) => stop());
    };
  }, []);
}

/** Takes the blur off the page once the player has closed or given up. */
export function usePlayerBackdrop() {
  useEffect(() => {
    const unlisten = onPlayerEvent((event) => {
      if (event.kind === "closed" || event.kind === "failed") {
        document.body.classList.remove(PLAYER_OPEN_CLASS);
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
}
