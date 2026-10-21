import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RankedStream, Stream, StreamResults } from "./streams";

export type PlayerEvent =
  | { kind: "loaded"; label: string }
  | { kind: "fallback"; label: string; reason: string }
  | { kind: "ended" }
  | { kind: "failed"; message: string }
  | { kind: "closed" };

/** A stream to try: a web link, or a torrent for the built-in engine. */
export interface Candidate {
  label: string;
  /** Identifies the stream across visits; see `streamKey`. */
  key: string;
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
      key: streamKey(s),
      url: s.url,
      torrent:
        !s.url && s.infoHash
          ? { infoHash: s.infoHash, fileIdx: s.fileIdx, sources: s.sources }
          : null,
    }));
}

/**
 * A stable name for a stream, saved with the video's progress so resuming starts on the same
 * source. Torrents are named by their hash and file; links by addon and file name. Never the
 * URL itself: debrid links carry the user's API key.
 */
export function streamKey(s: Stream): string {
  if (s.infoHash) return `bt:${s.infoHash.toLowerCase()}:${s.fileIdx ?? ""}`;
  const file = s.filename ?? `${s.name ?? ""} ${s.title ?? ""}`.replace(/\s+/g, " ").trim();
  return `${s.addonName}|${file}`;
}

/** Moves the stream saved as `key` to the front, looking in the filtered-out streams too
 * because the user may have picked one of those. Otherwise the order stays. */
export function preferSaved(results: StreamResults, key: string | null | undefined) {
  if (!key) return results.streams;
  const saved =
    results.streams.find((s) => streamKey(s) === key) ??
    results.hidden.find((s) => streamKey(s) === key);
  return saved ? [saved, ...results.streams.filter((s) => s !== saved)] : results.streams;
}

/** The title being played, for saving progress to the Stremio library. */
export interface TitleInfo {
  id: string;
  type: string;
  name: string;
  poster: string | null;
  /** Every video id in the addon's order; empty for movies. */
  videoIds: string[];
  /** Genres, used to tell anime from other series for skip times. */
  genres: string[];
}

/**
 * `item` and `videoId` identify what plays: for progress, resume and addon subtitles. `nextUp`
 * labels the next episode for the countdown near the end.
 */
export const openPlayer = (
  candidates: Candidate[],
  title: string,
  item: TitleInfo,
  videoId: string,
  nextUp: string | null,
) => invoke<void>("player_open", { candidates, title, item, videoId, nextUp });

/** Sent by the player's countdown when the next episode should start. */
export const PLAY_NEXT_EVENT = "player-play-next";

/**
 * Streams of the next episode with the same `bingeGroup` as the one just watched (same addon,
 * release and quality) move to the front, keeping the ranking otherwise.
 */
export function preferBingeGroup(ranked: RankedStream[], bingeGroup: string | null) {
  if (!bingeGroup) return ranked;
  const same = ranked.filter((s) => s.bingeGroup === bingeGroup);
  return [...same, ...ranked.filter((s) => s.bingeGroup !== bingeGroup)];
}

export const closePlayer = () => invoke<void>("player_close");

export const onPlayerEvent = (handler: (event: PlayerEvent) => void) =>
  listen<PlayerEvent>("player", (e) => handler(e.payload));
