import { invoke } from "@tauri-apps/api/core";

export interface LibraryItem {
  id: string;
  name: string;
  type: string;
  poster: string | null;
}

export interface Addon {
  id: string;
  name: string;
  version: string;
  description: string | null;
  logo: string | null;
  transportUrl: string;
  official: boolean;
  protected: boolean;
}

export const listLibrary = () => invoke<LibraryItem[]>("library_list");

export const listAddons = () => invoke<Addon[]>("addons_list");
