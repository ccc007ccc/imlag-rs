//! CFG mode — install a single dispatch cfg into the CS2 cfg directory and
//! rewrite it on each death event.
//!
//! Earlier versions of ImLag generated a fan-out of `imlag_say_*.cfg` files
//! and bound multiple keys, each pointing at a fixed line of corpus. That
//! exposed the player to **accidental disclosure**: any stray press of a
//! bound key would fire a preset message in chat.
//!
//! The current design keeps a **single** `imlag_say.cfg`, empty by default.
//! The trigger key is bound to `exec imlag_say`. When the player dies,
//! [`CfgManager::dispatch`] rewrites the cfg with a `say` / `say_team` line,
//! presses the trigger, waits long enough for CS2 to finish exec, then
//! truncates the cfg back to a comment-only stub. Pressing the trigger
//! between deaths is harmless.

use crate::chat::ChatMessageManager;
use crate::config::{AppConfig, CfgChatMode};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

const AUTOEXEC_BACKUP_SUFFIX: &str = ".imlag_backup";
const COMMENT_START: &str = "// --- ImLag Auto-Bind Start ---";
const COMMENT_END: &str = "// --- ImLag Auto-Bind End ---";
const SAY_CFG_FILE: &str = "imlag_say.cfg";
const EMPTY_SAY_CFG_BODY: &str =
    "// ImLag dispatch slot — content is rewritten at runtime.\n// Pressing the trigger key while this file is empty is a no-op.\n";

/// How long to wait between rewriting the cfg and clearing it again.
///
/// CS2 needs to read the file after we trigger `exec` from outside; a short
/// sleep avoids racing the engine. 300 ms covers worst-case console
/// processing while still allowing a roughly 3-deaths-per-second cadence.
const DISPATCH_CLEAR_DELAY: Duration = Duration::from_millis(300);

/// Errors produced while reading or writing CS2 cfg files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CfgError {
    /// The CS2 install directory could not be located.
    #[error("CS2 install directory not found")]
    Cs2NotFound,
    /// The CS2 install directory exists but `game/csgo/cfg` is missing.
    #[error("CS2 cfg directory not found at {0}")]
    CfgDirMissing(PathBuf),
    /// The corpus is empty so nothing can be dispatched.
    #[error("corpus is empty")]
    EmptyCorpus,
    /// Underlying I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// CFG-mode controller. Wires together [`AppConfig`] and the corpus
/// ([`ChatMessageManager`]) to manage the single dispatch cfg CS2 reads.
#[derive(Clone)]
pub struct CfgManager {
    config: Arc<parking_lot::RwLock<AppConfig>>,
    /// Serialises concurrent calls to [`Self::dispatch`] so we never have two
    /// threads writing or clearing the cfg at once.
    dispatch_lock: Arc<parking_lot::Mutex<()>>,
    #[allow(dead_code)]
    corpus: ChatMessageManager,
}

impl CfgManager {
    /// Create a controller bound to the given shared config and corpus.
    pub fn new(config: Arc<parking_lot::RwLock<AppConfig>>, corpus: ChatMessageManager) -> Self {
        Self {
            config,
            dispatch_lock: Arc::new(parking_lot::Mutex::new(())),
            corpus,
        }
    }

    /// Resolve the absolute path of the CS2 cfg directory.
    ///
    /// Preference order:
    ///  1. `config.cs2_path` if it points at a valid install,
    ///  2. cs2-gsi's Steam discovery.
    pub fn resolve_cfg_dir(&self) -> Result<PathBuf, CfgError> {
        let user_path = self.config.read().cs2_path.trim().to_string();
        if !user_path.is_empty() {
            let candidate = PathBuf::from(&user_path)
                .join("game")
                .join("csgo")
                .join("cfg");
            if candidate.is_dir()
                || PathBuf::from(&user_path)
                    .join("game/csgo/pak01_dir.vpk")
                    .is_file()
            {
                fs::create_dir_all(&candidate)?;
                return Ok(candidate);
            }
        }
        cs2_gsi::steam::find_cs2_cfg_dir().map_err(|_| CfgError::Cs2NotFound)
    }

    /// Detect the install directory and remember it in `config.cs2_path`.
    /// Returns the path that was stored.
    pub fn auto_detect_cs2_install(&self) -> Result<PathBuf, CfgError> {
        let install = cs2_gsi::steam::find_cs2_install_dir().map_err(|_| CfgError::Cs2NotFound)?;
        self.config.write().cs2_path = install.to_string_lossy().into();
        Ok(install)
    }

    /// Decide whether the next dispatch should target team chat.
    ///
    /// Reads the configured [`CfgChatMode`]. For [`CfgChatMode::Random`] a
    /// fresh coin flip is performed on every call.
    pub fn select_in_team_chat(&self) -> bool {
        match self.config.read().cfg_chat_mode {
            CfgChatMode::Global => false,
            CfgChatMode::Team => true,
            CfgChatMode::Random => fastrand::bool(),
        }
    }

    /// Install the dispatch cfg + autoexec bind. Idempotent — calling it
    /// repeatedly leaves the cfg directory in the same state.
    ///
    /// Replaces the legacy preset-pool layout: any stale `imlag_say_*.cfg`
    /// or selector files from prior versions are also cleared.
    pub fn install(&self) -> Result<(), CfgError> {
        let cfg_dir = self.resolve_cfg_dir()?;
        if !cfg_dir.is_dir() {
            return Err(CfgError::CfgDirMissing(cfg_dir));
        }
        clear_legacy_files(&cfg_dir)?;
        write_empty_say_cfg(&cfg_dir)?;
        self.update_autoexec()?;
        Ok(())
    }

    /// Patch `autoexec.cfg` so the configured trigger key fires
    /// `exec imlag_say`. A backup is taken on the first run.
    pub fn update_autoexec(&self) -> Result<(), CfgError> {
        let cfg_dir = self.resolve_cfg_dir()?;
        let autoexec = cfg_dir.join("autoexec.cfg");
        let backup = cfg_dir.join(format!("autoexec.cfg{AUTOEXEC_BACKUP_SUFFIX}"));

        let mut lines: Vec<String> = if autoexec.is_file() {
            if !backup.is_file() {
                fs::copy(&autoexec, &backup)?;
            }
            fs::read_to_string(&autoexec)?
                .lines()
                .map(str::to_owned)
                .collect()
        } else {
            vec![
                "// Counter-Strike 2 Autoexec Configuration File".into(),
                "// Generated by ImLag".into(),
                String::new(),
            ]
        };

        remove_imlag_section(&mut lines);
        add_imlag_section(&mut lines, &self.config.read());
        fs::write(&autoexec, lines.join("\n") + "\n")?;
        Ok(())
    }

    /// `true` iff the dispatch cfg exists and the autoexec contains the
    /// ImLag-managed block.
    pub fn is_installed(&self) -> bool {
        let Ok(cfg_dir) = self.resolve_cfg_dir() else {
            return false;
        };
        if !cfg_dir.join(SAY_CFG_FILE).is_file() {
            return false;
        }
        match fs::read_to_string(cfg_dir.join("autoexec.cfg")) {
            Ok(s) => s.contains(COMMENT_START) && s.contains(COMMENT_END),
            Err(_) => false,
        }
    }

    /// Write a `say "..."` / `say_team "..."` line into the dispatch cfg,
    /// press the trigger key once, then clear the cfg back to a no-op stub.
    ///
    /// Calls are serialised by an internal mutex; if a second death lands
    /// while a previous dispatch is still running, the second call simply
    /// queues until the cfg is clean.
    pub fn dispatch(&self, message: &str, in_team_chat: bool) -> Result<(), CfgError> {
        let _guard = self.dispatch_lock.lock();

        let cfg_dir = self.resolve_cfg_dir()?;
        let cfg_path = cfg_dir.join(SAY_CFG_FILE);

        let snapshot = self.config.read().clone();
        let trigger_spec = if snapshot.trigger_key.trim().is_empty() {
            "ins".to_string()
        } else {
            snapshot.trigger_key.clone()
        };

        write_dispatch_line(&cfg_path, message, in_team_chat)?;

        // Release any keys the player might still be holding so the
        // game-side `bind` actually fires when we synthesise the press.
        crate::platform::release_all_keys();
        sleep(Duration::from_millis(40));

        match crate::platform::press_key_spec(&trigger_spec, Duration::from_millis(60)) {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!("trigger key spec '{trigger_spec}' is not recognised");
            }
            Err(e) => {
                // SendInput got rejected — bail out before clearing the
                // cfg so the user can press the trigger manually if they
                // really want to send.
                return Err(CfgError::Io(e));
            }
        }

        // Give CS2 a beat to finish exec'ing the file before we wipe it.
        sleep(DISPATCH_CLEAR_DELAY);

        write_empty_say_cfg(&cfg_dir)?;
        Ok(())
    }

    /// Restore CS2 to its pre-ImLag state — drop the dispatch cfg, any
    /// legacy cfg files from older versions, and roll back `autoexec.cfg`
    /// to its backup (if any).
    pub fn restore(&self) -> Result<(), CfgError> {
        let cfg_dir = self.resolve_cfg_dir()?;
        let autoexec = cfg_dir.join("autoexec.cfg");
        let backup = cfg_dir.join(format!("autoexec.cfg{AUTOEXEC_BACKUP_SUFFIX}"));
        if backup.is_file() {
            fs::copy(&backup, &autoexec)?;
            fs::remove_file(&backup)?;
        } else if autoexec.is_file() {
            let mut lines: Vec<String> = fs::read_to_string(&autoexec)?
                .lines()
                .map(str::to_owned)
                .collect();
            remove_imlag_section(&mut lines);
            fs::write(&autoexec, lines.join("\n") + "\n")?;
        }
        clear_legacy_files(&cfg_dir)?;
        let _ = fs::remove_file(cfg_dir.join(SAY_CFG_FILE));
        Ok(())
    }
}

fn escape_for_cfg(message: &str) -> String {
    message.replace('"', "\"\"").replace(';', "")
}

fn write_empty_say_cfg(cfg_dir: &Path) -> std::io::Result<()> {
    let path = cfg_dir.join(SAY_CFG_FILE);
    fs::write(path, EMPTY_SAY_CFG_BODY)
}

fn write_dispatch_line(path: &Path, message: &str, in_team_chat: bool) -> std::io::Result<()> {
    let escaped = escape_for_cfg(message);
    let verb = if in_team_chat { "say_team" } else { "say" };
    let mut f = fs::File::create(path)?;
    writeln!(f, "// ImLag dispatch — generated at runtime")?;
    writeln!(f, "{verb} \"{escaped}\"")?;
    Ok(())
}

/// Remove every imlag-generated cfg from previous installs (preset pool +
/// selectors) so install() leaves only the new single dispatch file behind.
fn clear_legacy_files(cfg_dir: &Path) -> std::io::Result<()> {
    let Ok(rd) = fs::read_dir(cfg_dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        // Keep the new dispatch slot — we'll rewrite it ourselves.
        if name == SAY_CFG_FILE {
            continue;
        }
        // Anything else with the imlag_say_ prefix is a stale preset.
        if name.starts_with("imlag_say_") && name.ends_with(".cfg") {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn remove_imlag_section(lines: &mut Vec<String>) {
    let mut start = None;
    let mut end = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t == COMMENT_START {
            start = Some(i);
        }
        if t == COMMENT_END && start.is_some() {
            end = Some(i);
            break;
        }
    }
    if let (Some(s), Some(e)) = (start, end) {
        lines.drain(s..=e);
    }
    // Stamp out any orphaned legacy lines that escaped the comment block
    // (older versions wrote selector exec calls outside the markers).
    lines.retain(|l| {
        let s = l.trim();
        !s.contains("exec imlag_say_global_selector")
            && !s.contains("exec imlag_say_team_selector")
            && !s.contains("exec imlag_say_selector")
            && !s.contains("imlag_do_global_say")
            && !s.contains("imlag_do_team_say")
    });
}

fn add_imlag_section(lines: &mut Vec<String>, cfg: &AppConfig) {
    let trigger = format_bind_token(&cfg.trigger_key);
    lines.push(String::new());
    lines.push(COMMENT_START.into());
    lines.push("// This block is automatically managed by ImLag.".into());
    lines.push("// The dispatch cfg is empty until ImLag rewrites it on a".into());
    lines.push("// death event, so pressing the trigger by hand is a no-op.".into());
    lines.push(format!("bind \"{trigger}\" \"exec imlag_say\""));
    lines.push(format!(
        "echo \"ImLag: '{trigger}' bound to imlag_say.cfg\""
    ));
    lines.push(COMMENT_END.into());
    lines.push(String::new());
    lines.retain(|l| !l.trim().eq_ignore_ascii_case("host_writeconfig"));
    lines.push("host_writeconfig".into());
}

/// Convert a key spec like `"k"` / `"ins"` / `"f5"` into the token CS2's
/// `bind` command expects. Single ASCII letters/digits stay lower-case;
/// named keys (Insert, Home, F-row, …) become upper-case.
fn format_bind_token(spec: &str) -> String {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return "INS".into();
    }
    if trimmed.chars().count() == 1 {
        let ch = trimmed.chars().next().unwrap();
        if ch.is_ascii_alphanumeric() {
            return ch.to_ascii_lowercase().to_string();
        }
    }
    trimmed.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_strips_semicolons_and_doubles_quotes() {
        assert_eq!(escape_for_cfg("hi; \"world\""), "hi \"\"world\"\"");
    }

    #[test]
    fn imlag_section_round_trips_with_trigger_key() {
        let cfg = AppConfig {
            trigger_key: "j".into(),
            ..AppConfig::default()
        };
        let mut lines = vec!["echo legacy".into()];
        add_imlag_section(&mut lines, &cfg);
        assert!(lines
            .iter()
            .any(|l| l.contains("bind \"j\" \"exec imlag_say\"")));
        remove_imlag_section(&mut lines);
        assert!(!lines.iter().any(|l| l.contains("imlag_say")));
        assert!(lines.iter().any(|l| l == "echo legacy"));
    }

    #[test]
    fn imlag_section_uppercases_named_keys_for_bind() {
        let cfg = AppConfig {
            trigger_key: "ins".into(),
            ..AppConfig::default()
        };
        let mut lines = Vec::new();
        add_imlag_section(&mut lines, &cfg);
        // CS2's bind expects named keys upper-cased.
        assert!(lines
            .iter()
            .any(|l| l.contains("bind \"INS\" \"exec imlag_say\"")));
    }

    #[test]
    fn format_bind_token_handles_letters_and_named_keys() {
        assert_eq!(format_bind_token("k"), "k");
        assert_eq!(format_bind_token("K"), "k");
        assert_eq!(format_bind_token("ins"), "INS");
        assert_eq!(format_bind_token("F5"), "F5");
        assert_eq!(format_bind_token(""), "INS");
    }

    #[test]
    fn remove_imlag_section_strips_orphaned_legacy_lines() {
        let mut lines = vec![
            "echo before".into(),
            "exec imlag_say_global_selector".into(),
            "bind \"k\" \"imlag_do_global_say\"".into(),
            "echo after".into(),
        ];
        remove_imlag_section(&mut lines);
        assert_eq!(lines, vec!["echo before".to_string(), "echo after".into()]);
    }

    #[test]
    fn write_dispatch_line_distinguishes_global_and_team() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SAY_CFG_FILE);

        write_dispatch_line(&path, "lag bro", false).unwrap();
        let global = fs::read_to_string(&path).unwrap();
        assert!(global.contains("say \"lag bro\""));
        assert!(!global.contains("say_team"));

        write_dispatch_line(&path, "ez", true).unwrap();
        let team = fs::read_to_string(&path).unwrap();
        assert!(team.contains("say_team \"ez\""));
    }

    #[test]
    fn write_empty_say_cfg_produces_comment_only_body() {
        let dir = tempfile::tempdir().unwrap();
        write_empty_say_cfg(dir.path()).unwrap();
        let body = fs::read_to_string(dir.path().join(SAY_CFG_FILE)).unwrap();
        assert!(body.contains("dispatch slot"));
        assert!(!body.contains("say "));
    }

    #[test]
    fn clear_legacy_files_removes_pool_but_keeps_dispatch_slot() {
        let dir = tempfile::tempdir().unwrap();
        // Stale preset-pool files.
        fs::write(dir.path().join("imlag_say_global_1.cfg"), "say a").unwrap();
        fs::write(dir.path().join("imlag_say_team_2.cfg"), "say_team b").unwrap();
        fs::write(dir.path().join("imlag_say_global_selector.cfg"), "alias").unwrap();
        // Current dispatch slot.
        fs::write(dir.path().join(SAY_CFG_FILE), "// keep").unwrap();
        // Unrelated cfg.
        fs::write(dir.path().join("user_keybinds.cfg"), "bind w +forward").unwrap();

        clear_legacy_files(dir.path()).unwrap();

        assert!(!dir.path().join("imlag_say_global_1.cfg").is_file());
        assert!(!dir.path().join("imlag_say_team_2.cfg").is_file());
        assert!(!dir.path().join("imlag_say_global_selector.cfg").is_file());
        assert!(dir.path().join(SAY_CFG_FILE).is_file());
        assert!(dir.path().join("user_keybinds.cfg").is_file());
    }
}
