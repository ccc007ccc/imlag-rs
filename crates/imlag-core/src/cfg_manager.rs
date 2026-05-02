//! CFG mode — generate per-message `.cfg` files and patch `autoexec.cfg` so
//! pressing one key in CS2 cycles through the corpus.

use crate::chat::ChatMessageManager;
use crate::config::AppConfig;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const AUTOEXEC_BACKUP_SUFFIX: &str = ".imlag_backup";
const COMMENT_START: &str = "// --- ImLag Auto-Bind Start ---";
const COMMENT_END: &str = "// --- ImLag Auto-Bind End ---";

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
    /// The corpus is empty so nothing can be generated.
    #[error("corpus is empty")]
    EmptyCorpus,
    /// Underlying I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// CFG-mode controller. Wires together [`AppConfig`] and the corpus
/// ([`ChatMessageManager`]) to generate the family of `.cfg` files CS2 reads.
#[derive(Clone)]
pub struct CfgManager {
    config: Arc<parking_lot::RwLock<AppConfig>>,
    corpus: ChatMessageManager,
}

impl CfgManager {
    /// Create a controller bound to the given shared config and corpus.
    pub fn new(config: Arc<parking_lot::RwLock<AppConfig>>, corpus: ChatMessageManager) -> Self {
        Self { config, corpus }
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

    /// Pick a random global-chat key out of `config.bind_keys`.
    pub fn random_global_key(&self) -> String {
        let cfg = self.config.read();
        pick_random(&cfg.bind_keys).unwrap_or_else(|| "k".into())
    }

    /// Pick a random team-chat key out of `config.team_bind_keys`.
    pub fn random_team_key(&self) -> String {
        let cfg = self.config.read();
        pick_random(&cfg.team_bind_keys).unwrap_or_else(|| "l".into())
    }

    /// Generate the `imlag_say_global_*.cfg` / `imlag_say_team_*.cfg` files
    /// and the two selector cfgs.
    pub fn generate_files(&self) -> Result<usize, CfgError> {
        let messages = self.corpus.all();
        if messages.is_empty() {
            return Err(CfgError::EmptyCorpus);
        }
        let cfg_dir = self.resolve_cfg_dir()?;
        if !cfg_dir.is_dir() {
            return Err(CfgError::CfgDirMissing(cfg_dir));
        }

        delete_generated(&cfg_dir)?;

        let mut shuffled = messages.clone();
        shuffle_in_place(&mut shuffled);
        for (i, msg) in shuffled.iter().enumerate() {
            write_cfg_line(
                &cfg_dir,
                &format!("imlag_say_global_{}.cfg", i + 1),
                "ImLag Global Chat CFG",
                msg,
                "say",
            )?;
        }

        let mut shuffled = messages.clone();
        shuffle_in_place(&mut shuffled);
        for (i, msg) in shuffled.iter().enumerate() {
            write_cfg_line(
                &cfg_dir,
                &format!("imlag_say_team_{}.cfg", i + 1),
                "ImLag Team Chat CFG",
                msg,
                "say_team",
            )?;
        }

        write_selector(&cfg_dir, "global", messages.len())?;
        write_selector(&cfg_dir, "team", messages.len())?;
        Ok(messages.len())
    }

    /// Patch `autoexec.cfg` so the configured keys cycle through the
    /// generated say cfgs. A backup is taken on the first run.
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

    /// Convenience: [`generate_files`](Self::generate_files) followed by
    /// [`update_autoexec`](Self::update_autoexec).
    pub fn apply(&self) -> Result<usize, CfgError> {
        let n = self.generate_files()?;
        self.update_autoexec()?;
        Ok(n)
    }

    /// Restore CS2 to its pre-ImLag state — drops every `imlag_*.cfg` and
    /// rolls back `autoexec.cfg` to its backup (if any).
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
        for entry in fs::read_dir(&cfg_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(s) = name.to_str() {
                if s.starts_with("imlag_") && s.ends_with(".cfg") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }

    /// Number of generated say-cfg pairs (`min(global_count, team_count)`).
    pub fn generated_group_count(&self) -> usize {
        let cfg_dir = match self.resolve_cfg_dir() {
            Ok(p) => p,
            Err(_) => return 0,
        };
        let mut g = 0usize;
        let mut t = 0usize;
        if let Ok(rd) = fs::read_dir(&cfg_dir) {
            for e in rd.flatten() {
                if let Some(name) = e.file_name().to_str() {
                    if name.ends_with("_selector.cfg") {
                        continue;
                    }
                    if name.starts_with("imlag_say_global_") && name.ends_with(".cfg") {
                        g += 1;
                    }
                    if name.starts_with("imlag_say_team_") && name.ends_with(".cfg") {
                        t += 1;
                    }
                }
            }
        }
        g.min(t)
    }
}

fn pick_random(items: &[String]) -> Option<String> {
    if items.is_empty() {
        None
    } else {
        Some(items[fastrand::usize(..items.len())].clone())
    }
}

fn shuffle_in_place(v: &mut [String]) {
    for i in (1..v.len()).rev() {
        v.swap(i, fastrand::usize(..=i));
    }
}

fn escape_for_cfg(message: &str) -> String {
    message.replace('"', "\"\"").replace(';', "")
}

fn write_cfg_line(
    cfg_dir: &Path,
    file_name: &str,
    header: &str,
    msg: &str,
    verb: &str,
) -> std::io::Result<()> {
    let escaped = escape_for_cfg(msg);
    let mut f = fs::File::create(cfg_dir.join(file_name))?;
    writeln!(f, "// {header}")?;
    writeln!(f, "// Message: {msg}")?;
    writeln!(f, "{verb} \"{escaped}\"")?;
    Ok(())
}

fn write_selector(cfg_dir: &Path, kind: &str, count: usize) -> std::io::Result<()> {
    if count == 0 {
        return Ok(());
    }
    let path = cfg_dir.join(format!("imlag_say_{kind}_selector.cfg"));
    let mut f = fs::File::create(path)?;
    writeln!(f, "// ImLag {} Chat Selector CFG", capitalize(kind))?;
    writeln!(f, "// Cycles through {count} {kind} message CFGs.")?;
    writeln!(f)?;
    for i in 1..=count {
        let next = (i % count) + 1;
        writeln!(
            f,
            "alias imlag_{kind}_say_{i} \"exec imlag_say_{kind}_{i}; alias imlag_do_{kind}_say imlag_{kind}_say_{next}\""
        )?;
    }
    writeln!(f)?;
    writeln!(f, "alias imlag_do_{kind}_say imlag_{kind}_say_1")?;
    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn delete_generated(cfg_dir: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(cfg_dir)? {
        let entry = entry?;
        if let Some(s) = entry.file_name().to_str() {
            if s.ends_with("_selector.cfg") {
                continue;
            }
            if (s.starts_with("imlag_say_global_") || s.starts_with("imlag_say_team_"))
                && s.ends_with(".cfg")
            {
                let _ = fs::remove_file(entry.path());
            }
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
    lines.retain(|l| {
        let s = l.trim();
        !s.contains("exec imlag_say_global_selector")
            && !s.contains("exec imlag_say_team_selector")
            && !s.contains("exec imlag_say_selector")
    });
}

fn add_imlag_section(lines: &mut Vec<String>, cfg: &AppConfig) {
    lines.push(String::new());
    lines.push(COMMENT_START.into());
    lines.push("// This block is automatically managed by ImLag".into());
    lines.push("exec imlag_say_global_selector".into());
    lines.push("exec imlag_say_team_selector".into());
    for k in &cfg.bind_keys {
        lines.push(format!("bind \"{k}\" \"imlag_do_global_say\""));
        lines.push(format!("echo \"ImLag: '{k}' bound to global chat.\""));
    }
    for k in &cfg.team_bind_keys {
        lines.push(format!("bind \"{k}\" \"imlag_do_team_say\""));
        lines.push(format!("echo \"ImLag: '{k}' bound to team chat.\""));
    }
    lines.push(COMMENT_END.into());
    lines.push(String::new());
    lines.retain(|l| !l.trim().eq_ignore_ascii_case("host_writeconfig"));
    lines.push("host_writeconfig".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_strips_semicolons_and_doubles_quotes() {
        assert_eq!(escape_for_cfg("hi; \"world\""), "hi \"\"world\"\"");
    }

    #[test]
    fn imlag_section_is_round_trippable() {
        let cfg = AppConfig::default();
        let mut lines = vec!["echo legacy".into()];
        add_imlag_section(&mut lines, &cfg);
        assert!(lines
            .iter()
            .any(|l| l.contains("exec imlag_say_global_selector")));
        remove_imlag_section(&mut lines);
        assert!(!lines
            .iter()
            .any(|l| l.contains("imlag_say_global_selector")));
        assert!(lines.iter().any(|l| l == "echo legacy"));
    }
}
