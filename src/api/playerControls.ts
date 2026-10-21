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
  | { action: "toggleFullscreen" }
  | { action: "back" }
  | { action: "close" };

export const control = (c: Control) => invoke<void>("player_control", { control: c });

export const currentState = () => invoke<PlaybackState | null>("player_state");

export const onPlaybackState = (handler: (state: PlaybackState) => void) =>
  listen<PlaybackState>("player-state", (e) => handler(e.payload));
