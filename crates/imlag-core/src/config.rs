//! Persistent application configuration.
//!
//! Mirrors the structure of the original Godot/C# `AppConfig` — same field
//! names where reasonable, so existing `Config.json` files keep working.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Channel selection strategy for CFG mode.
///
/// CFG mode rewrites a single dispatch cfg at runtime; this enum decides
/// whether the produced line is `say "..."` (global), `say_team "..."`
/// (team), or randomised on each death.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CfgChatMode {
    /// Always send to global chat (`say`).
    #[default]
    Global,
    /// Always send to team chat (`say_team`).
    Team,
    /// Pick global or team randomly per death.
    Random,
}

/// Persistent settings for the ImLag application.
///
/// `serde` writes camelCase keys (matching the JS / TS frontend) but
/// also accepts the legacy Godot `PascalCase` and the early imlag-rs
/// `snake_case` spellings via `alias`, so old `Config.json` files keep
/// loading after upgrade.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// SteamIDs / display names of the players whose deaths should trigger a
    /// chat reply when [`only_self_death`](Self::only_self_death) is `true`.
    #[serde(default, alias = "PlayerNames", alias = "player_names")]
    pub player_names: Vec<String>,

    /// If `true`, only fire when one of [`player_names`](Self::player_names)
    /// is the dying player.
    #[serde(
        default = "default_true",
        alias = "OnlySelfDeath",
        alias = "only_self_death"
    )]
    pub only_self_death: bool,

    /// **Deprecated** — kept for backward-compatible loading of old configs.
    /// New code should read [`trigger_key`](Self::trigger_key) instead.
    #[serde(default, alias = "BindKeys", alias = "bind_keys")]
    pub bind_keys: Vec<String>,

    /// **Deprecated** — kept for backward-compatible loading of old configs.
    /// New code should read [`trigger_key`](Self::trigger_key) plus
    /// [`cfg_chat_mode`](Self::cfg_chat_mode) instead.
    #[serde(default, alias = "TeamBindKeys", alias = "team_bind_keys")]
    pub team_bind_keys: Vec<String>,

    /// Single key bound to `exec imlag_say` in CFG mode. The cfg is empty
    /// outside a death event, so pressing this key by accident is harmless.
    ///
    /// Serde default is the empty string so legacy configs without a
    /// `triggerKey` field migrate from `bindKeys` / `teamBindKeys` in
    /// [`Self::normalize`]; brand-new configs ride the [`Default`] impl
    /// which seeds it with `"k"`.
    #[serde(default, alias = "TriggerKey", alias = "trigger_key")]
    pub trigger_key: String,

    /// Channel selection strategy for CFG mode dispatches.
    #[serde(
        default,
        alias = "CfgChatMode",
        alias = "cfg_chat_mode",
        alias = "ChatMode"
    )]
    pub cfg_chat_mode: CfgChatMode,

    /// Filesystem path to the Counter-Strike 2 install directory. Empty when
    /// auto-detection is desired.
    #[serde(default, alias = "CS2Path", alias = "cs2_path")]
    pub cs2_path: String,

    /// `true` → use the CFG (bind-key) mode. `false` → simulate keys to type
    /// directly into the CS2 chat box.
    #[serde(default = "default_true", alias = "UseCfgMode", alias = "use_cfg_mode")]
    pub use_cfg_mode: bool,

    /// Key that opens the chat box in CS2 (`"y"` for global, `"u"` for team).
    #[serde(default = "default_chat_key", alias = "ChatKey", alias = "chat_key")]
    pub chat_key: String,

    /// Skip the foreground-window check before sending a message.
    #[serde(default, alias = "SkipWindowCheck", alias = "skip_window_check")]
    pub skip_window_check: bool,

    /// Hammer the chat-open key three times instead of once. Helps when CS2
    /// drops the first keystroke.
    #[serde(default, alias = "ForceMode", alias = "force_mode")]
    pub force_mode: bool,

    /// Inter-key delay in milliseconds (clamped to 30..=1000).
    #[serde(default = "default_key_delay", alias = "KeyDelay", alias = "key_delay")]
    pub key_delay: u32,

    /// UI language (`"zh-CN"` or `"zh-TW"`).
    #[serde(default = "default_language", alias = "Language")]
    pub language: String,

    /// Auto-start the GSI listener on launch.
    #[serde(
        default = "default_true",
        alias = "AutoStartGsi",
        alias = "auto_start_gsi"
    )]
    pub auto_start_gsi: bool,

    /// **Deprecated** — superseded by [`cfg_chat_mode`](Self::cfg_chat_mode).
    /// `true` is migrated to [`CfgChatMode::Team`] on first load.
    #[serde(default, alias = "PreferTeamChat", alias = "prefer_team_chat")]
    pub prefer_team_chat: bool,
}

fn default_true() -> bool {
    true
}
fn default_trigger_key() -> String {
    "k".into()
}
fn default_chat_key() -> String {
    "y".into()
}
fn default_key_delay() -> u32 {
    100
}
fn default_language() -> String {
    "zh-CN".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            player_names: Vec::new(),
            only_self_death: true,
            bind_keys: Vec::new(),
            team_bind_keys: Vec::new(),
            trigger_key: default_trigger_key(),
            cfg_chat_mode: CfgChatMode::default(),
            cs2_path: String::new(),
            use_cfg_mode: true,
            chat_key: default_chat_key(),
            skip_window_check: false,
            force_mode: false,
            key_delay: default_key_delay(),
            language: default_language(),
            auto_start_gsi: true,
            prefer_team_chat: false,
        }
    }
}

impl AppConfig {
    /// Load `Config.json` from `dir`, returning the default config if the
    /// file is missing or malformed.
    pub fn load_from(dir: &Path) -> Self {
        let path = dir.join("Config.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str::<Self>(&s) {
                Ok(mut cfg) => {
                    cfg.normalize();
                    cfg
                }
                Err(e) => {
                    tracing::warn!("invalid Config.json ({e}); falling back to defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Save `Config.json` next to the binary / in `dir`.
    pub fn save_to(&self, dir: &Path) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("Config.json");
        let json = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into());
        std::fs::write(&path, json)?;
        Ok(path)
    }

    /// Coerce out-of-range / nonsense values back into safe defaults and
    /// migrate deprecated fields into their replacements.
    pub fn normalize(&mut self) {
        // Migrate legacy multi-key pools into `trigger_key`.
        if self.trigger_key.trim().is_empty() {
            self.trigger_key = self
                .bind_keys
                .iter()
                .chain(self.team_bind_keys.iter())
                .find_map(|k| {
                    let t = k.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                })
                .unwrap_or_else(default_trigger_key);
        }
        // Migrate `prefer_team_chat=true` into `CfgChatMode::Team` once.
        if self.prefer_team_chat && matches!(self.cfg_chat_mode, CfgChatMode::Global) {
            self.cfg_chat_mode = CfgChatMode::Team;
        }

        if self.chat_key.trim().is_empty() {
            self.chat_key = default_chat_key();
        }
        if !(30..=1000).contains(&self.key_delay) {
            self.key_delay = default_key_delay();
        }
        self.language = normalize_language(&self.language);
        // Constrain trigger_key to a single ASCII alphanumeric character.
        let raw = self.trigger_key.trim().to_ascii_lowercase();
        let valid = raw
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_string());
        self.trigger_key = valid.unwrap_or_else(default_trigger_key);
    }
}

/// Normalise an arbitrary language tag down to the three we support.
pub fn normalize_language(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "zh-tw" | "zh-hk" | "zh-mo" => "zh-TW".into(),
        s if s.starts_with("en") => "en".into(),
        _ => "zh-CN".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips_through_json() {
        let cfg = AppConfig::default();
        let s = serde_json::to_string(&cfg).unwrap();
        let parsed: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.trigger_key, "k");
        assert_eq!(parsed.cfg_chat_mode, CfgChatMode::Global);
        assert!(parsed.only_self_death);
        assert_eq!(parsed.key_delay, 100);
    }

    #[test]
    fn legacy_pascal_case_keys_still_load() {
        // Original Godot/C# config used PascalCase keys with multi-key pools.
        let json = r#"{
            "PlayerNames": ["alice"],
            "OnlySelfDeath": true,
            "BindKeys": ["j","k"],
            "TeamBindKeys": ["l"],
            "CS2Path": "C:/Steam/cs2",
            "UseCfgMode": false,
            "ChatKey": "u",
            "KeyDelay": 200,
            "Language": "zh-TW",
            "AutoStartGsi": false,
            "PreferTeamChat": true
        }"#;
        let mut cfg: AppConfig = serde_json::from_str(json).unwrap();
        cfg.normalize();
        assert_eq!(cfg.player_names, vec!["alice"]);
        // First entry of the legacy pool wins.
        assert_eq!(cfg.trigger_key, "j");
        // PreferTeamChat → CfgChatMode::Team
        assert_eq!(cfg.cfg_chat_mode, CfgChatMode::Team);
        assert_eq!(cfg.cs2_path, "C:/Steam/cs2");
        assert!(!cfg.use_cfg_mode);
        assert_eq!(cfg.chat_key, "u");
        assert_eq!(cfg.key_delay, 200);
        assert_eq!(cfg.language, "zh-TW");
    }

    #[test]
    fn cfg_chat_mode_serializes_lowercase() {
        let json = r#"{"cfgChatMode":"random"}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.cfg_chat_mode, CfgChatMode::Random);
    }

    #[test]
    fn normalize_clamps_key_delay() {
        let mut cfg = AppConfig {
            key_delay: 9999,
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.key_delay, 100);
    }

    #[test]
    fn normalize_falls_back_when_trigger_key_is_garbage() {
        let mut cfg = AppConfig {
            trigger_key: "@@@".into(),
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.trigger_key, "k");
    }

    #[test]
    fn normalize_picks_first_legacy_bind_key_when_trigger_empty() {
        let mut cfg = AppConfig {
            trigger_key: String::new(),
            bind_keys: vec!["m".into()],
            ..Default::default()
        };
        cfg.normalize();
        assert_eq!(cfg.trigger_key, "m");
    }
}
