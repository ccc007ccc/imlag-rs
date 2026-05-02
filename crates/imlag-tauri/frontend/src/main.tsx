import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initI18n } from "@/lib/i18n";
import "./styles/globals.css";

// i18n needs to know the current language *before* React paints, so we
// pull the language from the engine's persisted config first. Failure
// here is non-fatal — the i18n module ships a sensible default.
initI18n().finally(() => {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
