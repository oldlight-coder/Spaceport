import { invoke } from "@tauri-apps/api/core";

export interface Episode {
  id: string;
  title: string;
  season: number | null;
  episode: number | null;
  released: string | null;
  thumbnail: string | null;
  overview: string | null;
}

export interface Season {
  number: number;
  episodes: Episode[];
}

export interface MetaDetails {
  id: string;
  type: string;
  name: string;
  poster: string | null;
  background: string | null;
  logo: string | null;
  description: string | null;
  releaseInfo: string | null;
  runtime: string | null;
  imdbRating: string | null;
  genres: string[];
  directors: string[];
  cast: string[];
  seasons: Season[];
  videos: Episode[];
  /** Every video id in the addon's order, which Stremio's watched marks follow. */
  videoIds: string[];
}

export const getMeta = (type: string, id: string, preferred?: string) =>
  invoke<MetaDetails>("meta_details", { kind: type, id, preferred: preferred ?? null });
