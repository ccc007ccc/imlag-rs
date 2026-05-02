import { useState } from "react";
import { Button, Card, Input, ModeOption } from "@/components";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";
import { api } from "@/lib/api";
import type { CfgChatMode } from "@/lib/types";

const MODE_ORDER: CfgChatMode[] = ["global", "team", "random"];

export function CfgModeView() {
  const { config, patchConfig } = useEngine();
  const { t } = useT();
  const [pathDraft, setPathDraft] = useState<string | null>(null);
  const [keyDraft, setKeyDraft] = useState<string | null>(null);

  if (!config) return null;

  const path = pathDraft ?? config.cs2Path;
  const trigger = keyDraft ?? config.triggerKey;

  const applyPath = async () => {
    await patchConfig("cs2Path", path.trim());
    setPathDraft(null);
  };

  const detectPath = async () => {
    // Reserved for a future Tauri command — auto-detect lives in CfgManager.
    console.warn("auto-detect not wired up yet");
  };

  const applyTriggerKey = async () => {
    const v = trigger.trim().toLowerCase();
    if (!v || !/^[a-z0-9]$/.test(v)) return;
    await patchConfig("triggerKey", v);
    setKeyDraft(null);
  };

  const onGenerate = async () => {
    try {
      await api.cfgGenerate();
    } catch (err) {
      console.error("[cfg] generate failed", err);
    }
  };

  const onRestore = async () => {
    try {
      await api.cfgRemove();
    } catch (err) {
      console.error("[cfg] remove failed", err);
    }
  };

  return (
    <div className="fluent-enter flex flex-col gap-4">
      <header>
        <h2 className="text-[20px] font-semibold text-fg-primary">
          {t("cfg.title")}
        </h2>
        <p className="text-[12px] text-fg-tertiary">{t("cfg.hint")}</p>
      </header>

      <Card title={t("cfg.cs2_path")}>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={path}
            onChange={(e) => setPathDraft(e.target.value)}
            placeholder="C:\\Program Files (x86)\\Steam\\steamapps\\common\\Counter-Strike Global Offensive"
            className="min-w-[320px] flex-1"
          />
          <Button variant="accent" onClick={applyPath}>
            {t("cfg.apply_path")}
          </Button>
          <Button variant="standard" onClick={detectPath}>
            {t("cfg.detect_path")}
          </Button>
        </div>
      </Card>

      <Card title={t("cfg.trigger_key")}>
        <p className="mb-2 text-[12px] text-fg-secondary">
          {t("cfg.trigger_hint")}
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={trigger}
            maxLength={1}
            onChange={(e) => setKeyDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                applyTriggerKey();
              }
            }}
            className="w-20! text-center font-mono"
            placeholder="k"
          />
          <Button variant="accent" size="sm" onClick={applyTriggerKey}>
            {t("cfg.apply_key")}
          </Button>
        </div>
      </Card>

      <Card title={t("cfg.mode.section")}>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
          {MODE_ORDER.map((mode) => (
            <ModeOption
              key={mode}
              selected={config.cfgChatMode === mode}
              label={t(`cfg.mode.${mode}`)}
              description={t(`cfg.mode.${mode}_description`)}
              onSelect={() => patchConfig("cfgChatMode", mode)}
            />
          ))}
        </div>
      </Card>

      <div className="flex justify-end gap-2">
        <Button variant="standard" onClick={onRestore}>
          {t("cfg.restore")}
        </Button>
        <Button variant="accent" onClick={onGenerate}>
          {t("cfg.generate")}
        </Button>
      </div>
    </div>
  );
}
