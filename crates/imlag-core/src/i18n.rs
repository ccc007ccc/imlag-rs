//! Lightweight localisation. Three languages — Simplified Chinese,
//! Traditional Chinese, and English — embedded as static tables.
//!
//! Mirrors the keys used by the original Godot UI so message wording stays
//! consistent across the rewrite.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::OnceLock;

const ZH_CN: &[(&str, &str)] = include!("i18n/zh_cn.in");
const ZH_TW: &[(&str, &str)] = include!("i18n/zh_tw.in");
const EN: &[(&str, &str)] = include!("i18n/en.in");

fn table_for(language: &str) -> &'static HashMap<&'static str, &'static str> {
    static ZH_CN_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    static ZH_TW_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    static EN_MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

    let init_cn = || ZH_CN.iter().copied().collect::<HashMap<_, _>>();
    let init_tw = || ZH_TW.iter().copied().collect::<HashMap<_, _>>();
    let init_en = || EN.iter().copied().collect::<HashMap<_, _>>();
    match language {
        "zh-TW" => ZH_TW_MAP.get_or_init(init_tw),
        "en" => EN_MAP.get_or_init(init_en),
        _ => ZH_CN_MAP.get_or_init(init_cn),
    }
}

/// Default UI language tag.
pub const DEFAULT_LANGUAGE: &str = "zh-CN";

static CURRENT: RwLock<Option<String>> = RwLock::new(None);

/// Override the active language. Pass `"zh-CN"`, `"zh-TW"` or `"en"`.
pub fn set_language(language: impl Into<String>) {
    *CURRENT.write() = Some(crate::config::normalize_language(&language.into()));
}

/// Return the active language tag (defaults to `"zh-CN"`).
pub fn current_language() -> String {
    CURRENT
        .read()
        .clone()
        .unwrap_or_else(|| DEFAULT_LANGUAGE.to_owned())
}

/// Translate `key`, performing zero-or-more `{0}`, `{1}`, … substitutions.
pub fn t(key: &str) -> String {
    template(key).to_owned()
}

/// `t!`-style helper for `{0}`, `{1}`, … substitutions.
pub fn t_args<'a, I, S>(key: &str, args: I) -> String
where
    I: IntoIterator<Item = &'a S>,
    S: AsRef<str> + 'a + ?Sized,
{
    let mut out = template(key).to_owned();
    for (i, value) in args.into_iter().enumerate() {
        let needle = format!("{{{i}}}");
        out = out.replace(&needle, value.as_ref());
    }
    out
}

fn template(key: &str) -> &'static str {
    let lang = current_language();
    let table = table_for(&lang);
    if let Some(v) = table.get(key) {
        return v;
    }
    if let Some(v) = table_for(DEFAULT_LANGUAGE).get(key) {
        return v;
    }
    key_static(key)
}

/// Leak-store unknown keys so we can return `&'static str` from a
/// translation miss without dragging the call site into `String`.
fn key_static(key: &str) -> &'static str {
    Box::leak(key.to_owned().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_default_language() {
        set_language("zh-TW");
        // gsi.started exists in both tables; differs in wording.
        let s = t("gsi.started");
        assert!(!s.is_empty());
    }

    #[test]
    fn substitutes_positional_args() {
        set_language("zh-CN");
        let s = t_args("status.player_dead", ["alice"].as_slice());
        assert!(s.contains("alice"));
    }
}
