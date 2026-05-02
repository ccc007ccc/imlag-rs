// Mirror of the Rust serialization shapes (camelCase wire format).
// Field names match exactly what `serde` emits — keep in sync with
// `imlag_core::AppConfig`, `imlag_tauri::commands::StatsSummary`,
// `imlag_tauri::events::UiEventDto`.

export type CfgChatMode = "global" | "team" | "random";

export interface AppConfig {
  playerNames: string[];
  onlySelfDeath: boolean;
  /** Single key bound to `exec imlag_say` in CFG mode. */
  triggerKey: string;
  /** Channel selection strategy for CFG dispatches. */
  cfgChatMode: CfgChatMode;
  /** @deprecated Replaced by `triggerKey`; kept for legacy config files. */
  bindKeys: string[];
  /** @deprecated Replaced by `triggerKey` + `cfgChatMode`. */
  teamBindKeys: string[];
  cs2Path: string;
  useCfgMode: boolean;
  chatKey: string;
  skipWindowCheck: boolean;
  forceMode: boolean;
  keyDelay: number;
  language: string;
  autoStartGsi: boolean;
  /** @deprecated Migrated into `cfgChatMode`. */
  preferTeamChat: boolean;
}

export interface ImportResult {
  added: number;
  skipped: number;
}

export type UiLevel = "info" | "warn" | "error";

export type UiKind =
  | "gsi"
  | "playerDeath"
  | "chatSent"
  | "cfg"
  | "corpus"
  | "config"
  | "other";

export interface UiEventDto {
  timestampMs: number;
  level: UiLevel;
  kind: UiKind;
  message: string;
}

export interface StatsSummary {
  corpusCount: number;
  cfgInstalled: boolean;
}

export type Tab = "general" | "cfg" | "chat" | "corpus";

export type LangCode = "zh-CN" | "zh-TW" | "en";
