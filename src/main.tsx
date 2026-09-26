import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Note: @heroui/react v3 (the installed version) ships no HeroUIProvider —
// its RAC-based components manage their own overlay/focus state, so the app
// renders without a global provider. See the delivery summary.

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
