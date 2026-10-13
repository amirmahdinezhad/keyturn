import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppState } from "./types";

/** Typed invoke wrappers for the backend commands.
 *
 * CLI-only contract: no login, no keys, no accounts. Auth lives in the
 * provider CLIs; we only read their status (`cliStatus`) and render the
 * login/install guidance.
 */

/** Read preferences and reconcile launch-at-login with the OS setting. */
export function getState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}

/** Request launch-at-login and return the resulting OS-backed state. */
export function setSettings(openAtLogin: boolean): Promise<AppState> {
  return invoke<AppState>("set_settings", { openAtLogin });
}

/** Merge refresh cadence or quota wording into persisted backend preferences. */
export function setAppSettings(
  settings: Partial<AppSettings>,
): Promise<AppState> {
  return invoke<AppState>("set_app_settings", { settings });
}

/** Exit the native application, including its tray icon. */
export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** CLI-only provider status (T3-style, read-only). Backend never logs in. */
export interface ProviderCliStatus {
  id: string;
  label: string;
  binary: string;
  binaryFound: boolean;
  binaryPath?: string | null;
  version?: string | null;
  loggedIn?: boolean | null;
  authMethod?: string | null;
  email?: string | null;
  account?: string | null;
  detail?: string | null;
  loginCmd: string;
  installUrl: string;
  authFile?: string | null;
  connected?: string[];
}

/** Probe every supported CLI without running installation or sign-in commands. */
export function cliStatus(): Promise<ProviderCliStatus[]> {
  return invoke<ProviderCliStatus[]>("cli_status");
}

/** A quota bucket whose percent always represents used capacity. */
export interface QuotaWindow {
  window: string;
  percent: number;
  resetsAt?: string | null;
  status?: string | null;
}

/** Provider limits or a source error, with observation time for local snapshots. */
export interface QuotaSet {
  provider: string;
  windows?: QuotaWindow[] | null;
  error?: string | null;
  asOf?: string | null;
}

/** Read all available quota sources; callers filter the result for their popup. */
export function cliQuotas(): Promise<QuotaSet[]> {
  return invoke<QuotaSet[]>("cli_quotas");
}

/** Strip the Tauri invoke prefix so backend validation messages read cleanly. */
export function prettyError(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  return raw.replace(/^Error invoking remote method '.*': (Error: )?/, "");
}
