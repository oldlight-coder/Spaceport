import { invoke } from "@tauri-apps/api/core";
import type { TitleInfo } from "./player";

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

/** Whether the title is in the user's Stremio library. */
export const libraryStatus = (id: string) => invoke<boolean>("library_status", { id });

/** Sent on the window after a title was added to or removed from the library. */
export const LIBRARY_CHANGED = "spaceport-library-changed";

/** Adds the title to the library or removes it; resolves to whether it is in it now. */
export async function setInLibrary(item: TitleInfo, add: boolean) {
  const now = await invoke<boolean>("library_set", { item, add });
  window.dispatchEvent(new Event(LIBRARY_CHANGED));
  return now;
}
