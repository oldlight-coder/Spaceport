import { invoke } from "@tauri-apps/api/core";
import { Appearance } from "../theme";

export interface StreamFilters {
  maxSize: number | null;
  minResolution: number | null;
  blockedCodecs: string[];
}

export interface SubtitleStyle {
  /** Size relative to the default, 0.5 to 2. */
  scale: number;
  /** "#RRGGBB" */
  color: string;
  background: "outline" | "box";
  /** Vertical position in percent of the screen height; 100 is the bottom. */
  position: number;
}

/** Row order and hidden rows, each catalog named "addonId/type/catalogId". */
export interface CatalogLayout {
  order: string[];
  hidden: string[];
}

export interface Settings {
  audioLanguages: string[];
  subtitleLanguages: string[];
  preferredResolution: number;
  filters: StreamFilters;
  avatar: string | null;
  torrentDht: boolean;
  torrentCacheGb: number;
  /** Android: play torrents only on Wi-Fi. */
  torrentWifiOnly: boolean;
  subtitleStyle: SubtitleStyle;
  /** Skip anime intros, outros and recaps without asking. */
  animeAutoSkip: boolean;
  /** Start the next episode when the countdown runs out. */
  autoPlayNext: boolean;
  appearance: Appearance;
  catalogLayout: CatalogLayout;
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
