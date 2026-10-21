// Recent searches, newest first, kept on this computer only.
const KEY = "spaceport.searchHistory";
const LIMIT = 20;

export function readHistory(): string[] {
  try {
    const list = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((t) => typeof t === "string") : [];
  } catch {
    return [];
  }
}

function write(list: string[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    // Without storage the history only lasts this session.
  }
  return list;
}

/** Puts `term` first, dropping an earlier copy that differs only in case. */
export function addToHistory(term: string) {
  const same = (t: string) => t.toLowerCase() === term.toLowerCase();
  return write([term, ...readHistory().filter((t) => !same(t))].slice(0, LIMIT));
}

export const removeFromHistory = (term: string) =>
  write(readHistory().filter((t) => t !== term));

export const clearHistory = () => write([]);
