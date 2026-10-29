// Build-time fixture bridge: production application imports remain untouched.
let handler: (cmd: string, args?: any) => Promise<any> = async () => null;
export function setFixtureHandler(next: typeof handler) { handler = next; }
export function invoke<T>(cmd: string, args?: any): Promise<T> { return handler(cmd, args); }
export function isTauri() { return "__TAURI_INTERNALS__" in window; }
export async function listen() { return () => {}; }
export async function getVersion() { return "0.1.0-motion-qa"; }
export async function check() { return null; }
export async function relaunch() {}
export async function openUrl() {}
