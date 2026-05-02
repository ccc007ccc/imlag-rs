import { useSyncExternalStore } from "react";
import zhCN from "@/locales/zh-CN.json";
import zhTW from "@/locales/zh-TW.json";
import en from "@/locales/en.json";
import { api } from "./api";
import type { LangCode } from "./types";

type Dict = Record<string, string>;

const dicts: Record<LangCode, Dict> = {
  "zh-CN": zhCN as Dict,
  "zh-TW": zhTW as Dict,
  en: en as Dict,
};

/** 标题栏语言下拉菜单使用的展示顺序。 */
export const LANG_CYCLE: readonly LangCode[] = ["zh-CN", "zh-TW", "en"];

let currentLang: LangCode = "zh-CN";
const listeners = new Set<() => void>();

function notify() {
  for (const fn of listeners) fn();
}

function normalize(raw: string): LangCode {
  if (raw === "zh-TW") return "zh-TW";
  if (raw.toLowerCase().startsWith("en")) return "en";
  return "zh-CN";
}

/**
 * Pull the persisted language out of the engine config. Called once
 * from `main.tsx` *before* React paints, so the very first render
 * already has the right strings.
 */
export async function initI18n(): Promise<void> {
  try {
    const cfg = await api.getConfig();
    currentLang = normalize(cfg.language);
  } catch (err) {
    console.warn("[i18n] could not read engine config, defaulting", err);
  }
}

export function getLanguage(): LangCode {
  return currentLang;
}

/**
 * Switch language. Persists through the engine, syncs `imlag_core::i18n`,
 * and re-renders every subscribed component.
 */
export async function setLanguage(lang: LangCode): Promise<void> {
  await api.setLanguage(lang);
  currentLang = lang;
  notify();
}

/**
 * Translate a key. Missing keys fall back to the zh-CN dictionary, then
 * to the raw key, so a typo never produces a blank string.
 *
 * Positional placeholders: `{0}`, `{1}` … are replaced by `args[i]`.
 */
export function t(key: string, ...args: (string | number)[]): string {
  let template = dicts[currentLang][key] ?? dicts["zh-CN"][key] ?? key;
  for (let i = 0; i < args.length; i++) {
    template = template.replaceAll(`{${i}}`, String(args[i]));
  }
  return template;
}

/**
 * Hook variant — re-renders the calling component whenever the active
 * language changes. The returned `t` is stable but reads `currentLang`
 * at call time, so it always reflects the latest dictionary.
 */
export function useT() {
  const lang = useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
    () => currentLang,
    () => currentLang,
  );
  return { lang, t, setLanguage };
}
