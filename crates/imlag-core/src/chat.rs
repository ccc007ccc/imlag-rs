//! Corpus management — load, persist, import / export the chat-message pool.

use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Default file name of the corpus.
pub const MESSAGES_FILE: &str = "Messages.txt";

/// Default Chinese corpus shipped with ImLag (matches the original Godot
/// project's `Messages.txt`).
pub const DEFAULT_MESSAGES: &[&str] = &[
    "网卡",
    "手抖",
    "高延迟",
    "鼠标出问题了",
    "瓶颈期",
    "手冻僵了",
    "被阴了",
    "卡输入法了",
    "掉帧了",
    "手汗手滑",
    "腱鞘炎犯了",
    "吞子弹了",
    "timing侠",
    "唉，资本",
    "刚打瓦回来不适应",
    "灵敏度有问题",
    "谁把我键位改了",
    "感冒了没反应",
    "校园网是这样的",
    "状态不行",
    "鼠标撞键盘上了",
    "复健",
    "屏幕太小",
    "键盘坏了",
    "显示器延迟高",
    "对面锁了",
    "他静音",
    "在开车",
    "有延迟",
    "在上课",
    "刚来电话了",
    "帧率低",
    "第一把",
    "没开灯",
    "在上厕所",
    "刮台风了",
    "走神了",
    "表白被拒",
    "电脑死机了",
    "纯小子一个",
    "对面太阴",
    "外卖到了",
    "作业太多",
    "身体虚了",
    "键盘进水了",
    "吃太撑了",
    "晚饭吃多了",
    "刚🦌完",
    "灵敏度没改",
    "思路被打扰了",
    "鼠标没电了",
    "好久没玩了",
    "预判失误",
    "心情不好",
    "空气不行",
    "手骨折了",
    "边吃饭边打的",
    "被绿了",
    "糖心看多了",
    "周围太吵",
    "91在后台",
    "尿急",
    "口渴",
    "地震了",
    "刚睡醒",
    "心脏病犯了",
    "我妈叫我了",
    "设备不行",
    "对面开了",
    "运气太差",
    "手汗太多",
    "被室友打扰了",
    "打累了",
    "按键有问题",
    "刚黑客入侵了",
];
/// Outcome of an import operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// Newly added (non-duplicate, non-empty) messages.
    pub added: usize,
    /// Skipped because empty / duplicate.
    pub skipped: usize,
}

/// Chat-message corpus.
///
/// Cheap to clone — every clone shares the same backing store.
#[derive(Clone, Default)]
pub struct ChatMessageManager {
    inner: Arc<RwLock<Vec<String>>>,
}

impl ChatMessageManager {
    /// Construct an empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of messages currently in the corpus.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// `true` when the corpus is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    /// Snapshot of every message in insertion order.
    pub fn all(&self) -> Vec<String> {
        self.inner.read().clone()
    }

    /// Pick a uniformly random message. Returns `None` if the corpus is empty.
    pub fn random(&self) -> Option<String> {
        let guard = self.inner.read();
        if guard.is_empty() {
            return None;
        }
        let idx = fastrand::usize(..guard.len());
        Some(guard[idx].clone())
    }

    /// Add a single message. Returns `true` if it was new.
    pub fn add(&self, message: impl AsRef<str>) -> bool {
        let trimmed = message.as_ref().trim();
        if trimmed.is_empty() {
            return false;
        }
        let mut guard = self.inner.write();
        if guard.iter().any(|m| m == trimmed) {
            return false;
        }
        guard.push(trimmed.to_owned());
        true
    }

    /// Remove a message. Returns `true` if it existed.
    pub fn remove(&self, message: impl AsRef<str>) -> bool {
        let trimmed = message.as_ref().trim();
        let mut guard = self.inner.write();
        let before = guard.len();
        guard.retain(|m| m != trimmed);
        guard.len() != before
    }

    /// Replace the entire corpus with `messages`.
    pub fn replace_all<I, S>(&self, messages: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut guard = self.inner.write();
        guard.clear();
        for m in messages {
            let s = m.into();
            let trimmed = s.trim();
            if !trimmed.is_empty() && !guard.iter().any(|m| m == trimmed) {
                guard.push(trimmed.to_owned());
            }
        }
    }

    /// Bulk-import messages, skipping empty / duplicate values.
    pub fn import<I, S>(&self, messages: I) -> ImportResult
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut added = 0usize;
        let mut skipped = 0usize;
        let mut guard = self.inner.write();
        for raw in messages {
            let trimmed = raw.as_ref().trim();
            if trimmed.is_empty() || guard.iter().any(|m| m == trimmed) {
                skipped += 1;
                continue;
            }
            guard.push(trimmed.to_owned());
            added += 1;
        }
        ImportResult { added, skipped }
    }

    /// Load `Messages.txt` from `dir`. If the file is missing, seed the
    /// corpus with [`DEFAULT_MESSAGES`] and write it out.
    pub fn load_or_seed(&self, dir: &Path) -> std::io::Result<PathBuf> {
        let path = dir.join(MESSAGES_FILE);
        if path.is_file() {
            let content = std::fs::read_to_string(&path)?;
            self.replace_all(parse_text(&content));
        } else {
            self.replace_all(DEFAULT_MESSAGES.iter().map(|s| s.to_string()));
            std::fs::create_dir_all(dir)?;
            self.save_to(dir)?;
        }
        Ok(path)
    }

    /// Persist the corpus as a UTF-8 text file (one entry per line).
    pub fn save_to(&self, dir: &Path) -> std::io::Result<PathBuf> {
        let path = dir.join(MESSAGES_FILE);
        std::fs::write(&path, self.all().join("\n"))?;
        Ok(path)
    }

    /// Import from a file path. The contents may be either a JSON array of
    /// strings or one message per line.
    pub fn import_from_file(&self, path: &Path) -> std::io::Result<ImportResult> {
        let content = std::fs::read_to_string(path)?;
        Ok(self.import(parse_text(&content)))
    }

    /// Import from a remote URL. Same content rules as
    /// [`import_from_file`](Self::import_from_file).
    pub async fn import_from_url(&self, url: &str) -> anyhow::Result<ImportResult> {
        let resp = reqwest::get(url).await?.error_for_status()?;
        let body = resp.text().await?;
        Ok(self.import(parse_text(&body)))
    }

    /// Export the corpus to `path` as one message per line.
    pub fn export_to_file(&self, path: &Path) -> std::io::Result<usize> {
        let messages = self.all();
        std::fs::write(path, messages.join("\n"))?;
        Ok(messages.len())
    }
}

/// Parse arbitrary text into a list of trimmed, non-empty messages.
///
/// Accepts either a JSON array of strings or newline-separated lines.
pub fn parse_text(content: &str) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
            return parsed
                .into_iter()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    trimmed
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn add_skips_duplicates() {
        let m = ChatMessageManager::new();
        assert!(m.add("hi"));
        assert!(!m.add("hi"));
        assert!(!m.add("   hi   "));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn parse_text_handles_json_and_lines() {
        assert_eq!(
            parse_text(r#"["a","b"," "]"#),
            vec!["a".to_string(), "b".into()]
        );
        assert_eq!(
            parse_text("a\r\nb\nc\n\n"),
            vec!["a".to_string(), "b".into(), "c".into()]
        );
    }

    #[test]
    fn load_or_seed_creates_file_with_defaults() {
        let dir = TempDir::new().unwrap();
        let m = ChatMessageManager::new();
        m.load_or_seed(dir.path()).unwrap();
        assert!(dir.path().join(MESSAGES_FILE).is_file());
        assert_eq!(m.len(), DEFAULT_MESSAGES.len());
    }
}
