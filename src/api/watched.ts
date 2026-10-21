import { invoke } from "@tauri-apps/api/core";
import { TitleInfo } from "./player";

export interface WatchedMarks {
  /** The whole title is marked watched. */
  title: boolean;
  /** Watched video ids, for series. */
  videos: string[];
}

export const getWatched = (item: TitleInfo) => invoke<WatchedMarks>("watched_get", { item });

/** Marks `videos`, or the whole title when `videos` is empty, watched or unwatched. */
export const setWatched = (item: TitleInfo, videos: string[], watched: boolean) =>
  invoke<WatchedMarks>("watched_set", { item, videos, watched });
