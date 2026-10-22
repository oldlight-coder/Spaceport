import { invoke } from "@tauri-apps/api/core";

export interface CatalogRef {
  transportUrl: string;
  addonName: string;
  /** The addon's manifest id; names the catalog in the saved row layout. */
  addonId: string;
  type: string;
  id: string;
  name: string;
  genres: string[];
  genreRequired: boolean;
  supportsSkip: boolean;
}

export interface MetaPreview {
  id: string;
  type: string;
  name: string;
  poster: string | null;
  posterShape: string | null;
  /** Year or year range, when the addon gives one. */
  releaseInfo?: string | null;
}

/** A title the search assistant found, with the addon it came from. */
export interface SearchHit extends MetaPreview {
  transportUrl: string;
  addonName: string;
  /** How close the title is to the query, 0 to 1. */
  score: number;
}

/**
 * Titles close to `query` from every search catalog, best first, typos forgiven. `quick` is
 * for autocomplete: looser forms of the query are only tried when the query finds too little.
 */
export const searchAssist = (query: string, quick: boolean, limit: number) =>
  invoke<SearchHit[]>("search_assist", { query, quick, limit });

export const listCatalogs = () => invoke<CatalogRef[]>("catalogs_list");

export const searchCatalogs = () => invoke<CatalogRef[]>("search_catalogs");

export const fetchCatalog = (catalog: CatalogRef, extra: [string, string][] = []) =>
  invoke<MetaPreview[]>("catalog_fetch", {
    transportUrl: catalog.transportUrl,
    kind: catalog.type,
    id: catalog.id,
    extra,
  });

const TYPE_LABELS: Record<string, string> = {
  movie: "Movies",
  series: "Series",
  anime: "Anime",
  channel: "Channels",
  tv: "TV",
};

export const typeLabel = (type: string) =>
  TYPE_LABELS[type] ?? type.charAt(0).toUpperCase() + type.slice(1);
