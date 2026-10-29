import type { AppState } from "../types";

import { DEFAULT_APP_SETTINGS } from "../types";
export { DEFAULT_APP_SETTINGS } from "../types";

export function makeState(over: Partial<AppState> = {}): AppState {
  return {
    settings: { openAtLogin: false },
    app: DEFAULT_APP_SETTINGS,
    ...over,
  };
}
