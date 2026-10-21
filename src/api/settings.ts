import { invoke } from "@tauri-apps/api/core";

export interface StreamFilters {
  maxSize: number | null;
  minResolution: number | null;
  blockedCodecs: string[];
}

export interface Settings {
  audioLanguages: string[];
  subtitleLanguages: string[];
  preferredResolution: number;
  filters: StreamFilters;
  avatar: string | null;
  torrentDht: boolean;
  torrentCacheGb: number;
}

export const getSettings = () => invoke<Settings>("settings_get");

export const saveSettings = (settings: Settings) => invoke<Settings>("settings_set", { settings });

const languageNames = (() => {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" });
  } catch {
    return null;
  }
})();

export const languageName = (code: string) => languageNames?.of(code) ?? code.toUpperCase();

/** Languages offered in the preference lists (ISO 639-1). */
export const LANGUAGE_CODES = [
  "en", "de", "fr", "es", "it", "pt", "nl", "sv", "no", "da", "fi", "pl", "cs", "hu", "ro",
  "el", "tr", "ru", "uk", "ar", "he", "hi", "ja", "ko", "zh", "th", "vi", "id",
];

export const CODECS = ["AV1", "HEVC", "AVC", "VP9", "XviD"];
