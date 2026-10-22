import { invoke } from "@tauri-apps/api/core";

export interface VideoProgress {
  /** Seconds. */
  position: number;
  duration: number;
  updated: number;
  /** Key of the stream last played (`streamKey`), to resume on the same source. */
  source?: string | null;
}

export interface ContinueItem {
  id: string;
  type: string;
  name: string;
  poster: string | null;
  videoId: string | null;
  position: number;
  duration: number;
  lastWatched: string;
}

export const continueWatching = () => invoke<ContinueItem[]>("continue_watching");

/** Every title played, newest first (also ones no longer in the library). */
export const watchHistory = () => invoke<ContinueItem[]>("watch_history");

/** Takes titles off Continue Watching; they start from the beginning next time. */
export const dismissContinue = (ids: string[]) => invoke<void>("continue_dismiss", { ids });

/** Saved positions of every video of a title, by video id. */
export const titleProgress = (id: string) =>
  invoke<Record<string, VideoProgress>>("title_progress", { id });

/** "S1 E2" for an episode id like "tt0903747:1:2"; null for a movie. */
export function episodeLabel(videoId: string | null, titleId: string) {
  if (!videoId || videoId === titleId) return null;
  const parts = videoId.split(":");
  const [season, episode] = parts.slice(-2).map(Number);
  if (parts.length >= 3 && Number.isFinite(season) && Number.isFinite(episode)) {
    return `S${season} E${episode}`;
  }
  return null;
}

/** Share watched, 0 to 1, or null when there is nothing to show. */
export function watchedShare(p: { position: number; duration: number } | undefined) {
  if (!p || p.position <= 0 || p.duration <= 0) return null;
  return Math.min(1, p.position / p.duration);
}
