import { useEffect, useState } from "react";

// Titles the user marked as being watched now, newest first, kept on this computer. Stremio's
// library has no field for this, so it is not synced.
const KEY = "spaceport.watching";
const CHANGED = "spaceport-watching-changed";

export interface WatchingItem {
  id: string;
  type: string;
  name: string;
  poster: string | null;
}

export function readWatching(): WatchingItem[] {
  try {
    const list = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((i) => i && typeof i.id === "string") : [];
  } catch {
    return [];
  }
}

function write(list: WatchingItem[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // Without storage the marks only last this session.
  }
  window.dispatchEvent(new Event(CHANGED));
}

export function setWatching(item: WatchingItem, on: boolean) {
  const rest = readWatching().filter((i) => i.id !== item.id);
  write(on ? [item, ...rest] : rest);
}

/** The marked titles, updated whenever a mark changes anywhere in the window. */
export function useWatching() {
  const [list, setList] = useState(readWatching);
  useEffect(() => {
    const update = () => setList(readWatching());
    window.addEventListener(CHANGED, update);
    return () => window.removeEventListener(CHANGED, update);
  }, []);
  return list;
}
