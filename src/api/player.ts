import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RankedStream } from "./streams";

export type PlayerEvent =
  | { kind: "loaded"; label: string }
  | { kind: "fallback"; label: string; reason: string }
  | { kind: "ended" }
  | { kind: "failed"; message: string }
  | { kind: "closed" };

/** A stream to try: a web link, or a torrent for the built-in engine. */
export interface Candidate {
  label: string;
  url: string | null;
  torrent: { infoHash: string; fileIdx: number | null; sources: string[] } | null;
}

/** Fallbacks beyond this many are not worth waiting through. */
const MAX_CANDIDATES = 10;

/**
 * The streams to try, in order: `first` (when the user picked one) and then the rest of the
 * ranked list. Direct and debrid links play as they are; torrents go through the engine.
 */
export function candidatesFrom(ranked: RankedStream[], first?: RankedStream): Candidate[] {
  const ordered = first ? [first, ...ranked.filter((s) => s !== first)] : ranked;
  return ordered
    .filter((s) => s.url || s.infoHash)
    .slice(0, MAX_CANDIDATES)
    .map((s) => ({
      label: (s.name ?? s.addonName).replace(/\s+/g, " "),
      url: s.url,
      torrent:
        !s.url && s.infoHash
          ? { infoHash: s.infoHash, fileIdx: s.fileIdx, sources: s.sources }
          : null,
    }));
}

/** The title being played, for saving progress to the Stremio library. */
export interface TitleInfo {
  id: string;
  type: string;
  name: string;
  poster: string | null;
  /** Every video id in the addon's order; empty for movies. */
  videoIds: string[];
}

/** `item` and `videoId` identify what plays: for progress, resume and addon subtitles. */
export const openPlayer = (
  candidates: Candidate[],
  title: string,
  item: TitleInfo,
  videoId: string,
) => invoke<void>("player_open", { candidates, title, item, videoId });

export const closePlayer = () => invoke<void>("player_close");

export const onPlayerEvent = (handler: (event: PlayerEvent) => void) =>
  listen<PlayerEvent>("player", (e) => handler(e.payload));
