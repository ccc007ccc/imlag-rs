import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, ImportResult, StatsSummary } from "./types";

// Thin typed wrappers around Tauri's `invoke`. The frontend never calls
// `invoke` directly; it goes through this module so the command names
// and payload shapes are checked at compile time.
export const api = {
  // ── Configuration ──
  getConfig: (): Promise<AppConfig> => invoke("get_config"),
  updateConfig: (config: AppConfig): Promise<AppConfig> =>
    invoke("update_config", { config }),
  setLanguage: (language: string): Promise<string> =>
    invoke("set_language", { language }),

  // ── GSI lifecycle ──
  startGsi: (): Promise<void> => invoke("start_gsi"),
  stopGsi: (): Promise<void> => invoke("stop_gsi"),
  isGsiRunning: (): Promise<boolean> => invoke("is_gsi_running"),

  // ── Corpus ──
  corpusList: (): Promise<string[]> => invoke("corpus_list"),
  corpusAdd: (message: string): Promise<boolean> =>
    invoke("corpus_add", { message }),
  corpusRemove: (message: string): Promise<boolean> =>
    invoke("corpus_remove", { message }),
  corpusExport: (path: string): Promise<number> =>
    invoke("corpus_export", { path }),
  corpusImport: (path: string): Promise<ImportResult> =>
    invoke("corpus_import", { path }),

  // ── CFG mode ──
  cfgGenerate: (): Promise<number> => invoke("cfg_generate"),
  cfgRemove: (): Promise<void> => invoke("cfg_remove"),

  // ── Stats ──
  statsSummary: (): Promise<StatsSummary> => invoke("stats_summary"),
};
