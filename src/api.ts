import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppState, HistoryOut, UpdateOut } from "./types";

/** Typed invoke wrappers for the backend commands.
 *
 * CLI-only contract: no login, no keys, no accounts. Auth lives in the
 * provider CLIs; we only read their status (`cliStatus`) and render the
 * login/install guidance.
 */

export function getState(): Promise<AppState> {
  return invoke<AppState>("get_state");
}

export function setSettings(openAtLogin: boolean): Promise<AppState> {
  return invoke<AppState>("set_settings", { openAtLogin });
}

/** Merge a partial app-settings object (refresh cadence, billing, updates). */
export function setAppSettings(
  settings: Partial<AppSettings>,
): Promise<AppState> {
  return invoke<AppState>("set_app_settings", { settings });
}

export function history(range: string): Promise<HistoryOut> {
  return invoke<HistoryOut>("history", { range });
}

export function checkUpdates(): Promise<UpdateOut> {
  return invoke<UpdateOut>("check_updates");
}

export function exportHistory(range: string, format: "json" | "csv"): Promise<string> {
  return invoke<string>("export_history", { range, format });
}

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

export function cliStatus(): Promise<ProviderCliStatus[]> {
  return invoke<ProviderCliStatus[]>("cli_status");
}

/** Strip the Tauri invoke prefix so backend validation messages read cleanly. */
export function prettyError(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  return raw.replace(/^Error invoking remote method '.*': (Error: )?/, "");
}
