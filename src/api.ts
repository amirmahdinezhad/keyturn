import { invoke } from "@tauri-apps/api/core";
import type { AppSettings, AppState, CodexOut, HistoryOut, Month, UpdateOut } from "./types";

/** Typed invoke wrappers for the backend commands. */

export function getState(month: Month | null, refresh = false): Promise<AppState> {
  return invoke<AppState>("get_state", { month, refresh });
}

export interface SaveKeyArgs {
  provider: string;
  name: string;
  apiKey: string;
  account: string;
}

export function saveKey(args: SaveKeyArgs): Promise<AppState> {
  return invoke<AppState>("save_key", { ...args });
}

export function deleteKey(provider: string, name: string): Promise<AppState> {
  return invoke<AppState>("delete_key", { provider, name });
}

export function applyKey(provider: string, name: string): Promise<AppState> {
  return invoke<AppState>("apply_key", { provider, name });
}

export function addProvider(provider: string): Promise<AppState> {
  return invoke<AppState>("add_provider", { provider });
}

export function removeProvider(provider: string): Promise<AppState> {
  return invoke<AppState>("remove_provider", { provider });
}

export function setSettings(openAtLogin: boolean): Promise<AppState> {
  return invoke<AppState>("set_settings", { openAtLogin });
}

/** Merge a partial app-settings object (tray, notifications, automation). */
export function setAppSettings(
  settings: Partial<AppSettings>,
): Promise<AppState> {
  return invoke<AppState>("set_app_settings", { settings });
}

export function profileCreate(name: string): Promise<AppState> {
  return invoke<AppState>("profile_create", { name });
}

export function profileRename(id: string, name: string): Promise<AppState> {
  return invoke<AppState>("profile_rename", { id, name });
}

export function profileSwitch(id: string): Promise<AppState> {
  return invoke<AppState>("profile_switch", { id });
}

export function profileDelete(id: string): Promise<AppState> {
  return invoke<AppState>("profile_delete", { id });
}

export function listSounds(): Promise<string[]> {
  return invoke<string[]>("list_sounds");
}

export function playSound(name: string): Promise<void> {
  return invoke("play_sound", { name });
}

export function codexStatus(): Promise<CodexOut> {
  return invoke<CodexOut>("codex_status");
}

export function codexRefresh(): Promise<CodexOut> {
  return invoke<CodexOut>("codex_refresh");
}

export function history(range: string, key?: string | null): Promise<HistoryOut> {
  return invoke<HistoryOut>("history", { range, key: key ?? null });
}

export function checkUpdates(): Promise<UpdateOut> {
  return invoke<UpdateOut>("check_updates");
}

export function exportHistory(
  range: string,
  format: "json" | "csv",
  key?: string | null,
): Promise<string> {
  return invoke<string>("export_history", { range, format, key: key ?? null });
}

export interface AccountRef {
  id: string;
  provider: string;
  label?: string | null;
}

export interface AccountStatus {
  accounts: AccountRef[];
  activeAccount?: string | null;
  /** Diagnostics from the post-login console probe. */
  probe?: string;
}

export function accountStatus(): Promise<AccountStatus> {
  return invoke<AccountStatus>("account_status");
}

export function connectAccount(
  provider: string,
  account?: string | null,
): Promise<AccountStatus> {
  return invoke<AccountStatus>("connect_account_webview", { provider, account });
}

export function disconnectAccount(account: string): Promise<AccountStatus> {
  return invoke<AccountStatus>("disconnect_account", { account });
}

export function setActiveAccount(account: string): Promise<AccountStatus> {
  return invoke<AccountStatus>("set_active_account_cmd", { account });
}

export interface ConsoleKey {
  id: string;
  name?: string;
  prefix?: string;
  secret?: string;
  email?: string;
}

export interface ConsoleRow {
  model: string;
  keyId?: string;
  date?: string;
  cost?: number;
}

export interface GoWindow {
  label: string;
  percent: number;
  reset: string;
}

export interface ConsoleUsage {
  keys: ConsoleKey[];
  usage: ConsoleRow[];
  go: GoWindow[];
}

export interface ConsoleKeysOut {
  keys: ConsoleKey[];
  month: { year: number; month: number };
  email?: string | null;
}

export function consoleKeys(
  account: string,
  month: { year: number; month: number },
  refresh = false,
): Promise<ConsoleKeysOut> {
  return invoke<ConsoleKeysOut>("console_keys", { account, month, refresh });
}

export function consoleUsage(
  account: string,
  month: { year: number; month: number },
  refresh = false,
): Promise<ConsoleUsage> {
  return invoke<ConsoleUsage>("console_usage", { account, month, refresh });
}

export function activateAccountKey(keyId: string, account: string): Promise<AppState> {
  return invoke<AppState>("activate_account_key", { keyId, account });
}

export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** Strip the Tauri invoke prefix so backend validation messages read cleanly. */
export function prettyError(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e);
  return raw.replace(/^Error invoking remote method '.*': (Error: )?/, "");
}
