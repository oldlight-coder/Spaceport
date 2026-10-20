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

export interface AddonOutcome {
  addonName: string;
  transportUrl: string;
  count: number;
  error: string | null;
}

export interface StreamResults {
  streams: Stream[];
  addons: AddonOutcome[];
}

export const fetchStreams = (type: string, videoId: string) =>
  invoke<StreamResults>("streams_fetch", { kind: type, videoId });
