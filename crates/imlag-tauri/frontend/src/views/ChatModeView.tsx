import { useState } from "react";
import { Button, Card, Input, Notice, Toggle } from "@/components";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";

export function ChatModeView() {
  const { config, patchConfig } = useEngine();
  const { t } = useT();
  const [keyDraft, setKeyDraft] = useState<string | null>(null);
  const [delayDraft, setDelayDraft] = useState<string | null>(null);

  if (!config) return null;

  const chatKey = keyDraft ?? config.chatKey;
  const delay = delayDraft ?? String(config.keyDelay);

  const applyChatKey = async () => {
    const v = chatKey.trim();
    if (!v) return;
    await patchConfig("chatKey", v);
    setKeyDraft(null);
  };

  const applyDelay = async () => {
    const n = parseInt(delay.trim(), 10);
    if (Number.isNaN(n)) return;
    const clamped = Math.max(30, Math.min(1000, n));
    await patchConfig("keyDelay", clamped);
    setDelayDraft(String(clamped));
  };

  return (
    <div className="fluent-enter flex flex-col gap-4">
      <header>
        <h2 className="text-[20px] font-semibold text-fg-primary">
          {t("chat.title")}
        </h2>
        <p className="text-[12px] text-fg-tertiary">{t("chat.hint")}</p>
      </header>

      {config.useCfgMode && (
        <Notice tone="warning">{t("chat.inactive_notice")}</Notice>
      )}

      <Card>
        <div className="flex flex-wrap items-end gap-x-6 gap-y-3">
          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] text-fg-secondary">
              {t("chat.key")}
            </span>
            <div className="flex items-center gap-2">
              <Input
                value={chatKey}
                onChange={(e) => setKeyDraft(e.target.value)}
                className="w-20! text-center font-mono"
                maxLength={1}
              />
              <Button variant="standard" size="sm" onClick={applyChatKey}>
                {t("chat.apply_key")}
              </Button>
            </div>
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-[12px] text-fg-secondary">
              {t("chat.delay")}
            </span>
            <div className="flex items-center gap-2">
              <Input
                value={delay}
                onChange={(e) => setDelayDraft(e.target.value)}
                className="w-24!"
                inputMode="numeric"
              />
              <Button variant="standard" size="sm" onClick={applyDelay}>
                {t("chat.apply_delay")}
              </Button>
            </div>
          </label>
        </div>
      </Card>

      <Card>
        <div className="flex flex-wrap gap-x-6 gap-y-3">
          <Toggle
            checked={config.skipWindowCheck}
            onChange={(v) => patchConfig("skipWindowCheck", v)}
            label={t("chat.skip_window_check")}
          />
          <Toggle
            checked={config.forceMode}
            onChange={(v) => patchConfig("forceMode", v)}
            label={t("chat.force_mode")}
          />
        </div>
      </Card>
    </div>
  );
}
