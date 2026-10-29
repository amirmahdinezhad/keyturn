import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import "./panel.css";
import { applyDisplayPrefs, loadDisplayPrefs } from "./displayPrefs";

// Paint theme + accent before first paint so the panel never flashes.
try {
  applyDisplayPrefs(loadDisplayPrefs());
} catch {
  // Non-DOM or private mode: App re-applies on mount.
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
