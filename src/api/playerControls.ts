import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Track {
  id: number;
  label: string;
  selected: boolean;
  /** Loaded from an addon rather than part of the file. */
  external: boolean;
}

export interface AddonSubtitle {
  key: string;
  label: string;
  addonName: string;
  selected: boolean;
}

export interface SkipSegment {
  kind: "intro" | "outro" | "recap";
  /** Seconds. */
  start: number;
  end: number;
}

export interface PlaybackState {
  title: string;
  stream: string;
  paused: boolean;
  buffering: boolean;
  position: number;
  duration: number;
  volume: number;
  muted: boolean;
  speed: number;
  seeking: boolean;
  subDelay: number;
  /** Anime intro, outro and recap times. */
  skips: SkipSegment[];
  autoSkip: boolean;
  /** Why an anime has no skip times, when the lookup stopped somewhere. */
  skipNote: string | null;
  /** The next episode's label, when there is one to count down to. */
  nextUp: string | null;
  /** Where playback resumed, when it did not start at the beginning. */
  resumedFrom: number | null;
  subScale: number;
  seekable: boolean;
  audioTracks: Track[];
  subtitleTracks: Track[];
  addonSubtitles: AddonSubtitle[];
  sources: string[];
  sourceIndex: number;
}

export type Control =
  | { action: "togglePause" }
  | { action: "seek"; seconds: number; relative: boolean }
  | { action: "setVolume"; volume: number }
  | { action: "toggleMute" }
  | { action: "setSpeed"; speed: number }
  | { action: "setAudioTrack"; id: number }
  | { action: "setSubtitle"; track: number | null; key: string | null }
  | { action: "setSource"; index: number }
  | { action: "adjustSubtitleDelay"; seconds: number }
  | { action: "resetSubtitleDelay" }
  | { action: "setSubtitleScale"; scale: number }
  | { action: "toggleFullscreen" }
  | { action: "back" }
  | { action: "close" };

export const control = (c: Control) => invoke<void>("player_control", { control: c });

/** Opens a file picker and loads the chosen subtitle file into the player. */
export const loadSubtitleFile = () => invoke<void>("player_load_subtitle");

export const currentState = () => invoke<PlaybackState | null>("player_state");

export const onPlaybackState = (handler: (state: PlaybackState) => void) =>
  listen<PlaybackState>("player-state", (e) => handler(e.payload));
