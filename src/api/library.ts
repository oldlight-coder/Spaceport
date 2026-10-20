import { invoke } from "@tauri-apps/api/core";

export interface LibraryItem {
  id: string;
  name: string;
  type: string;
  poster: string | null;
}

export const listLibrary = () => invoke<LibraryItem[]>("library_list");
