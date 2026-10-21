import { invoke } from "@tauri-apps/api/core";

export interface LibraryItem {
  id: string;
  name: string;
  type: string;
  poster: string | null;
  /** When it was added (ISO 8601). */
  added: string;
  /** When it was last watched (ISO 8601), if ever. */
  lastWatched: string | null;
}

export const listLibrary = () => invoke<LibraryItem[]>("library_list");
