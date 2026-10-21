import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RankedStream } from "./streams";

export type PlayerEvent =
  | { kind: "loaded"; label: string }
  | { kind: "fallback"; label: string; reason: string }
  | { kind: "ended" }
  | { kind: "failed"; message: string }
  | { kind: "closed" };

export interface Candidate {
  url: string;
  label: string;
}

/** Fallbacks beyond this many are not worth waiting through. */
const MAX_CANDIDATES = 10;

/**
 * The streams to try, in order: `first` (when the user picked one) and then the rest of the
 * ranked list. Only direct and debrid links for now; torrents need the engine from M5.
 */
export function candidatesFrom(ranked: RankedStream[], first?: RankedStream): Candidate[] {
  const ordered = first ? [first, ...ranked.filter((s) => s !== first)] : ranked;
  return ordered
    .filter((s) => s.url)
    .slice(0, MAX_CANDIDATES)
    .map((s) => ({ url: s.url!, label: (s.name ?? s.addonName).replace(/\s+/g, " ") }));
}

/** `type` and `videoId` identify the video, for fetching subtitles from addons. */
export const openPlayer = (candidates: Candidate[], title: string, type: string, videoId: string) =>
  invoke<void>("player_open", { candidates, title, kind: type, videoId });

export const closePlayer = () => invoke<void>("player_close");

export const onPlayerEvent = (handler: (event: PlayerEvent) => void) =>
  listen<PlayerEvent>("player", (e) => handler(e.payload));
