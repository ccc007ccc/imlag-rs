import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { api } from "./api";
import { onUiEvent } from "./events";
import type { AppConfig, StatsSummary } from "./types";

interface EngineState {
  config: AppConfig | null;
  corpus: string[];
  stats: StatsSummary;
  gsiRunning: boolean;
  /** Replace the entire config (server-side normalises and persists). */
  saveConfig(next: AppConfig): Promise<void>;
  /** Patch a single field — convenience around `saveConfig`. */
  patchConfig<K extends keyof AppConfig>(key: K, value: AppConfig[K]): Promise<void>;
  refresh(): Promise<void>;
}

const Ctx = createContext<EngineState | null>(null);

export function EngineProvider({ children }: { children: ReactNode }) {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [corpus, setCorpus] = useState<string[]>([]);
  const [stats, setStats] = useState<StatsSummary>({
    corpusCount: 0,
    cfgInstalled: false,
  });
  const [gsiRunning, setGsiRunning] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [c, list, s, r] = await Promise.all([
        api.getConfig(),
        api.corpusList(),
        api.statsSummary(),
        api.isGsiRunning(),
      ]);
      setConfig(c);
      setCorpus(list);
      setStats(s);
      setGsiRunning(r);
    } catch (err) {
      console.error("[engine] refresh failed", err);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // React to backend-side mutations (e.g. CFG mode buttons emitting
  // events on completion). Each kind only refreshes the slice it cares
  // about so we avoid thrashing the IPC boundary.
  useEffect(() => {
    const unsubP = onUiEvent((evt) => {
      switch (evt.kind) {
        case "gsi":
          api.isGsiRunning().then(setGsiRunning).catch(console.warn);
          break;
        case "corpus":
          Promise.all([api.corpusList(), api.statsSummary()])
            .then(([l, s]) => {
              setCorpus(l);
              setStats(s);
            })
            .catch(console.warn);
          break;
        case "cfg":
          api.statsSummary().then(setStats).catch(console.warn);
          break;
        case "config":
          api.getConfig().then(setConfig).catch(console.warn);
          break;
        default:
          break;
      }
    });
    return () => {
      unsubP.then((u) => u()).catch(() => undefined);
    };
  }, []);

  const saveConfig = useCallback(async (next: AppConfig) => {
    try {
      const persisted = await api.updateConfig(next);
      setConfig(persisted);
    } catch (err) {
      console.error("[engine] updateConfig failed", err);
    }
  }, []);

  const patchConfig = useCallback(
    async <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => {
      if (!config) return;
      const next = { ...config, [key]: value };
      await saveConfig(next);
    },
    [config, saveConfig],
  );

  const value = useMemo(
    () => ({ config, corpus, stats, gsiRunning, saveConfig, patchConfig, refresh }),
    [config, corpus, stats, gsiRunning, saveConfig, patchConfig, refresh],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useEngine(): EngineState {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useEngine() called outside <EngineProvider>");
  return ctx;
}
