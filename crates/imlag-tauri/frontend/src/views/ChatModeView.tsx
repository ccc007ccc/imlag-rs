import { useState } from "react";
import { Button, Card, Input, Toggle } from "@/components";
import { cn } from "@/lib/cn";
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
                className="!w-20 text-center font-mono"
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
                className="!w-24"
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

      <Card title={t("section.chat")}>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <ModeOption
            selected={config.useCfgMode}
            label={t("mode.cfg")}
            description={t("mode.cfg_description")}
            onSelect={() => patchConfig("useCfgMode", true)}
          />
          <ModeOption
            selected={!config.useCfgMode}
            label={t("mode.chat")}
            description={t("mode.chat_description")}
            onSelect={() => patchConfig("useCfgMode", false)}
          />
        </div>
      </Card>
    </div>
  );
}

interface ModeOptionProps {
  selected: boolean;
  label: string;
  description: string;
  onSelect(): void;
}

function ModeOption({ selected, label, description, onSelect }: ModeOptionProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      data-reveal
      className={cn(
        "rounded-md border px-4 py-3 text-left transition-colors cursor-default",
        "duration-(--duration-fast) ease-(--ease-fluent)",
        selected
          ? "bg-accent-tertiary border-[color:var(--color-accent-base)]/55"
          : "bg-fill-control border-stroke-control hover:bg-fill-control-hover",
      )}
    >
      <div className="flex items-center gap-2">
        <span
          className={cn(
            "inline-block h-3.5 w-3.5 rounded-full border",
            selected
              ? "border-[color:var(--color-accent-base)] bg-accent-base"
              : "border-stroke-control-strong",
          )}
        />
        <span
          className={cn(
            "text-[13px] font-semibold",
            selected ? "text-fg-accent" : "text-fg-primary",
          )}
        >
          {label}
        </span>
      </div>
      <p className="mt-1.5 text-[12px] text-fg-tertiary leading-snug">
        {description}
      </p>
    </button>
  );
}
