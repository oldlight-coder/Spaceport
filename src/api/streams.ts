import { invoke } from "@tauri-apps/api/core";

export interface ParsedStream {
  resolution: number | null;
  hdr: string[];
  codec: string | null;
  bitDepth: number | null;
  audio: string[];
  channels: string | null;
  sizeBytes: number | null;
  seeders: number | null;
  releaseGroup: string | null;
  source: string | null;
  cached: boolean | null;
  debrid: string | null;
  languages: string[];
  multiAudio: boolean;
}

export interface Stream {
  addonName: string;
  transportUrl: string;
  name: string | null;
  title: string | null;
  url: string | null;
  infoHash: string | null;
  fileIdx: number | null;
  sources: string[];
  ytId: string | null;
  externalUrl: string | null;
  filename: string | null;
  videoSize: number | null;
  bingeGroup: string | null;
  notWebReady: boolean;
  parsed: ParsedStream;
}

export interface ScorePart {
  reason: string;
  points: number;
}

/** A stream with its score; streams come back best first. */
export interface RankedStream extends Stream {
  score: { total: number; parts: ScorePart[] };
}

export interface AddonOutcome {
  addonName: string;
  transportUrl: string;
  count: number;
  error: string | null;
}

export interface HiddenStream extends RankedStream {
  hiddenBecause: string;
}

export interface StreamResults {
  streams: RankedStream[];
  /** Streams the user's filters removed, still ranked. */
  hidden: HiddenStream[];
  addons: AddonOutcome[];
}

/**
 * Why a stream search found nothing to play, for the picker and the Play button; null when
 * there are streams.
 */
export function noStreamsReason(results: StreamResults): string | null {
  if (results.streams.length > 0) return null;
  if (results.addons.length === 0) {
    return "None of your addons provides streams for this title. Install a stream addon.";
  }
  if (results.hidden.length > 0) return "Every stream was hidden by your filters.";
  // Each addon's own error is listed in the picker.
  if (results.addons.every((a) => a.error)) {
    return "None of your stream addons answered. Check your connection and try again.";
  }
  return "Your addons found no streams for this title.";
}

export const fetchStreams = (type: string, videoId: string) =>
  invoke<StreamResults>("streams_fetch", { kind: type, videoId });
