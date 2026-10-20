import { invoke } from "@tauri-apps/api/core";

export interface CatalogRef {
  transportUrl: string;
  addonName: string;
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
}

export const listCatalogs = () => invoke<CatalogRef[]>("catalogs_list");

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
