// Display prefs — frontend-only, persisted in localStorage.
// Backend owns refresh cadence + usage mode; everything here is purely
// presentational so it never touches the IPC contract.

/** Appearance selection, with system mode resolved against the OS. */
export type ThemeMode = "dark" | "light" | "system";

/** Frontend preferences stored independently of backend settings. */
export interface DisplayPrefs {
  /** Show the CLI version line under each provider. */
  showDetails: boolean;
  /** Tighter rows for small panels. */
  compactRows: boolean;
  /** Re-read state + CLI status when the panel regains focus. */
  refreshOnFocus: boolean;
  /** Run an update check automatically when the Updates tab opens. */
  checkUpdatesOnLaunch: boolean;
  /** Panel color scheme. `system` follows the OS. */
  theme: ThemeMode;
  /** Accent hex (e.g. #007aff). Tints primary actions, links, bars. */
  accent: string;
}

/** Storage key for the panel's frontend preferences. */
export const DISPLAY_PREFS_KEY = "keyturn.displayPrefs";

/** Default accent used when a saved color is absent or invalid. */
export const DEFAULT_ACCENT = "#007aff";

/** Curated accents — Apple-native blue default, muted CLI/ember/orchid alternates. */
export const ACCENT_PRESETS: Array<{ name: string; value: string; hint: string }> = [
  { name: "Signal blue", value: "#007aff", hint: "Default" },
  { name: "Moss", value: "#2f9e6e", hint: "Terminal" },
  { name: "Ember", value: "#c26a2b", hint: "Warm" },
  { name: "Orchid", value: "#7c6cf0", hint: "Violet" },
];

/** Initial appearance and opt-in refresh/update behavior. */
export const DEFAULT_DISPLAY_PREFS: DisplayPrefs = {
  showDetails: true,
  compactRows: false,
  // Panel opens show cached data instantly; refresh is manual or interval-only.
  refreshOnFocus: false,
  checkUpdatesOnLaunch: false,
  theme: "dark",
  accent: DEFAULT_ACCENT,
};

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** Load validated preferences, filling missing or invalid fields with defaults. */
export function loadDisplayPrefs(): DisplayPrefs {
  try {
    const raw = localStorage.getItem(DISPLAY_PREFS_KEY);
    if (!raw) return { ...DEFAULT_DISPLAY_PREFS };
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed)) return { ...DEFAULT_DISPLAY_PREFS };
    return {
      showDetails:
        typeof parsed.showDetails === "boolean"
          ? parsed.showDetails
          : DEFAULT_DISPLAY_PREFS.showDetails,
      compactRows:
        typeof parsed.compactRows === "boolean"
          ? parsed.compactRows
          : DEFAULT_DISPLAY_PREFS.compactRows,
      refreshOnFocus:
        typeof parsed.refreshOnFocus === "boolean"
          ? parsed.refreshOnFocus
          : DEFAULT_DISPLAY_PREFS.refreshOnFocus,
      checkUpdatesOnLaunch:
        typeof parsed.checkUpdatesOnLaunch === "boolean"
          ? parsed.checkUpdatesOnLaunch
          : DEFAULT_DISPLAY_PREFS.checkUpdatesOnLaunch,
      theme: isThemeMode(parsed.theme) ? parsed.theme : DEFAULT_DISPLAY_PREFS.theme,
      accent:
        typeof parsed.accent === "string" && isHexColor(parsed.accent)
          ? normalizeHex(parsed.accent)
          : DEFAULT_DISPLAY_PREFS.accent,
    };
  } catch {
    return { ...DEFAULT_DISPLAY_PREFS };
  }
}

/** Persist preferences when storage is available; storage errors are ignored. */
export function saveDisplayPrefs(prefs: DisplayPrefs): void {
  try {
    localStorage.setItem(DISPLAY_PREFS_KEY, JSON.stringify(prefs));
  } catch {
    // Private mode etc: prefs just won't persist.
  }
}

/** Clear saved preferences and appearance overrides, then return fresh defaults. */
export function resetDisplayPrefs(): DisplayPrefs {
  try {
    localStorage.removeItem(DISPLAY_PREFS_KEY);
  } catch {
    // Ignore — defaults still apply in memory.
  }
  try {
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.removeProperty("--accent");
    document.documentElement.style.removeProperty("--accent-hover");
    document.documentElement.style.removeProperty("--accent-bg");
  } catch {
    // Non-DOM (tests): nothing to clean up.
  }
  return { ...DEFAULT_DISPLAY_PREFS };
}

function isThemeMode(v: unknown): v is ThemeMode {
  return v === "dark" || v === "light" || v === "system";
}

function isHexColor(v: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(v.trim());
}

function normalizeHex(v: string): string {
  const t = v.trim().toLowerCase();
  if (/^#[0-9a-f]{3}$/.test(t)) {
    return `#${t[1]}${t[1]}${t[2]}${t[2]}${t[3]}${t[3]}`;
  }
  return t;
}

function hexToRgb(hex: string): [number, number, number] {
  const h = normalizeHex(hex).slice(1);
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

function mix(hex: string, target: [number, number, number], amount: number): string {
  const [r, g, b] = hexToRgb(hex);
  const m = (c: number, t: number) =>
    Math.round(c + (t - c) * amount)
      .toString(16)
      .padStart(2, "0");
  return `#${m(r, target[0])}${m(g, target[1])}${m(b, target[2])}`;
}

/** Resolve `system` against the OS; `dark`/`light` pass through. */
export function resolveTheme(mode: ThemeMode): "dark" | "light" {
  if (mode !== "system") return mode;
  try {
    return window.matchMedia("(prefers-color-scheme: light)").matches
      ? "light"
      : "dark";
  } catch {
    return "dark";
  }
}

/**
 * Apply theme + accent to <html>. Hover is the accent mixed 18% toward
 * white; the wash is the accent at 16% over transparent.
 */
export function applyDisplayPrefs(prefs: Pick<DisplayPrefs, "theme" | "accent">): void {
  try {
    const root = document.documentElement;
    const resolved = resolveTheme(prefs.theme);
    root.setAttribute("data-theme", resolved);
    root.style.colorScheme = resolved;
    const accent = isHexColor(prefs.accent)
      ? normalizeHex(prefs.accent)
      : DEFAULT_ACCENT;
    const [r, g, b] = hexToRgb(accent);
    root.style.setProperty("--accent", accent);
    root.style.setProperty("--accent-hover", mix(accent, [255, 255, 255], 0.18));
    root.style.setProperty("--accent-bg", `rgba(${r}, ${g}, ${b}, 0.16)`);
  } catch {
    // Non-DOM (tests): no-op.
  }
}
