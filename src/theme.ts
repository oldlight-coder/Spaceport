import {
  argbFromHex,
  DynamicColor,
  hexFromArgb,
  Hct,
  MaterialDynamicColors as roles,
  SchemeTonalSpot,
} from "@material/material-color-utilities";

export type ThemeMode = "system" | "light" | "dark";

export interface Appearance {
  mode: ThemeMode;
  /** "#RRGGBB" */
  accent: string;
}

export const DEFAULT_APPEARANCE: Appearance = { mode: "system", accent: "#7C8CFF" };

/** Accent presets offered in Settings, next to a free colour picker. */
export const ACCENT_PRESETS = [
  "#7C8CFF",
  "#1A73E8",
  "#00897B",
  "#43A047",
  "#F9A825",
  "#F4511E",
  "#E91E63",
  "#8E24AA",
];

// The Material 3 colour roles the stylesheets use, as `--md-<name>`.
const ROLES: [string, DynamicColor][] = [
  ["primary", roles.primary],
  ["on-primary", roles.onPrimary],
  ["primary-container", roles.primaryContainer],
  ["on-primary-container", roles.onPrimaryContainer],
  ["secondary-container", roles.secondaryContainer],
  ["on-secondary-container", roles.onSecondaryContainer],
  ["tertiary", roles.tertiary],
  ["tertiary-container", roles.tertiaryContainer],
  ["on-tertiary-container", roles.onTertiaryContainer],
  ["surface", roles.surface],
  ["surface-container-lowest", roles.surfaceContainerLowest],
  ["surface-container-low", roles.surfaceContainerLow],
  ["surface-container", roles.surfaceContainer],
  ["surface-container-high", roles.surfaceContainerHigh],
  ["surface-container-highest", roles.surfaceContainerHighest],
  ["on-surface", roles.onSurface],
  ["on-surface-variant", roles.onSurfaceVariant],
  ["outline", roles.outline],
  ["outline-variant", roles.outlineVariant],
  ["error", roles.error],
  ["on-error", roles.onError],
  ["error-container", roles.errorContainer],
  ["on-error-container", roles.onErrorContainer],
  ["inverse-surface", roles.inverseSurface],
  ["inverse-on-surface", roles.inverseOnSurface],
];

// "Watched" green is not a Material role; it gets fixed light and dark tones.
const SUCCESS = {
  dark: { success: "#8EE6B4", "success-container": "#173A2A", "success-outline": "#2F6B4C" },
  light: { success: "#1B6B45", "success-container": "#CDEFD9", "success-outline": "#7CC29B" },
};

const darkQuery = () => window.matchMedia?.("(prefers-color-scheme: dark)");

const isDark = (mode: ThemeMode) =>
  mode === "dark" || (mode === "system" && (darkQuery()?.matches ?? true));

/** The CSS variables for one appearance; `forceDark` is for the player over video. */
export function themeVariables(appearance: Appearance, forceDark = false) {
  const dark = forceDark || isDark(appearance.mode);
  let source: number;
  try {
    source = argbFromHex(appearance.accent);
  } catch {
    source = argbFromHex(DEFAULT_APPEARANCE.accent);
  }
  const scheme = new SchemeTonalSpot(Hct.fromInt(source), dark, 0);
  const vars: Record<string, string> = {};
  for (const [name, role] of ROLES) vars[`--md-${name}`] = hexFromArgb(role.getArgb(scheme));
  for (const [name, value] of Object.entries(dark ? SUCCESS.dark : SUCCESS.light)) {
    vars[`--md-${name}`] = value;
  }
  return { vars, dark };
}

/** Sets the colours on the page; returns a function that stops following the system. */
export function applyTheme(appearance: Appearance, forceDark = false): () => void {
  const paint = () => {
    const { vars, dark } = themeVariables(appearance, forceDark);
    const root = document.documentElement;
    for (const [name, value] of Object.entries(vars)) root.style.setProperty(name, value);
    root.style.colorScheme = dark ? "dark" : "light";
    root.dataset.theme = dark ? "dark" : "light";
  };
  paint();
  if (forceDark || appearance.mode !== "system") return () => {};
  const query = darkQuery();
  query?.addEventListener("change", paint);
  return () => query?.removeEventListener("change", paint);
}

// The last appearance is kept here too, so the next start paints it before settings load.
const CACHE_KEY = "spaceport.appearance";

export function cachedAppearance(): Appearance {
  try {
    const saved = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
    if (saved && typeof saved.accent === "string" && typeof saved.mode === "string") return saved;
  } catch {
    // Fall through to the default.
  }
  return DEFAULT_APPEARANCE;
}

export function cacheAppearance(appearance: Appearance) {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(appearance));
  } catch {
    // Only the first paint of the next start is affected.
  }
}
