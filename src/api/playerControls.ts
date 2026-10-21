import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface AudioTrack {
  id: number;
  label: string;
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
  audioTracks: AudioTrack[];
}

export type Control =
  | { action: "togglePause" }
  | { action: "seek"; seconds: number; relative: boolean }
  | { action: "setVolume"; volume: number }
  | { action: "toggleMute" }
  | { action: "setSpeed"; speed: number }
  | { action: "setAudioTrack"; id: number }
  | { action: "toggleFullscreen" }
  | { action: "back" }
  | { action: "close" };

export const control = (c: Control) => invoke<void>("player_control", { control: c });

export const currentState = () => invoke<PlaybackState | null>("player_state");

export const onPlaybackState = (handler: (state: PlaybackState) => void) =>
  listen<PlaybackState>("player-state", (e) => handler(e.payload));
