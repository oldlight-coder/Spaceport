import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PlayerEvent =
  | { kind: "loaded" }
  | { kind: "ended" }
  | { kind: "failed"; message: string }
  | { kind: "closed" };

export const openPlayer = (url: string, title: string) =>
  invoke<void>("player_open", { url, title });

export const closePlayer = () => invoke<void>("player_close");

export const onPlayerEvent = (handler: (event: PlayerEvent) => void) =>
  listen<PlayerEvent>("player", (e) => handler(e.payload));
