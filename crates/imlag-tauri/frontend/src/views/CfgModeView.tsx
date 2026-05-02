import { useState, type KeyboardEvent } from "react";
import { Button, Card, Input, Toggle } from "@/components";
import { cn } from "@/lib/cn";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";
import { api } from "@/lib/api";

type KeyKind = "global" | "team";

interface KeyEditorProps {
  kind: KeyKind;
  sample: string;
  summaryKey: "cfg.current_global_keys" | "cfg.current_team_keys";
}

function KeyEditor({ kind, sample, summaryKey }: KeyEditorProps) {
  const { config, patchConfig } = useEngine();
  const { t } = useT();
  const [draft, setDraft] = useState("");
  if (!config) return null;

  const list = kind === "global" ? config.bindKeys : config.teamBindKeys;
  const field = kind === "global" ? "bindKeys" : "teamBindKeys";

  const isValid = (s: string) =>
    s.length === 1 && /[a-z0-9]/.test(s);

  const add = async () => {
    const trimmed = draft.trim().toLowerCase();
    if (!isValid(trimmed)) return;
    if (list.includes(trimmed)) {
      setDraft("");
      return;
    }
    await patchConfig(field, [...list, trimmed]);
    setDraft("");
  };

  const remove = async (k: string) => {
    await patchConfig(field, list.filter((x) => x !== k));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      add();
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <p className="text-[12px] text-fg-secondary">
        {t(summaryKey, list.join(", "))}
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={draft}
          maxLength={1}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={t("cfg.key_placeholder", sample)}
          className="!w-24"
        />
        <Button variant="accent" size="sm" onClick={add}>
          {t("common.add")}
        </Button>

        <div className="ml-2 flex flex-wrap gap-1">
          {list.map((k) => (
            <button
              key={k}
              type="button"
              onClick={() => remove(k)}
              className={cn(
                "inline-flex h-6 min-w-6 items-center justify-center px-2",
                "text-[12px] font-mono rounded-sm border",
                "bg-fill-control border-stroke-control text-fg-primary",
                "hover:bg-error hover:text-fg-on-accent hover:border-transparent",
                "transition-colors duration-(--duration-fast) ease-(--ease-fluent)",
              )}
              title={t("common.delete")}
            >
              {k}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function CfgModeView() {
  const { config, patchConfig } = useEngine();
  const { t } = useT();
  const [pathDraft, setPathDraft] = useState<string | null>(null);

  if (!config) return null;

  const path = pathDraft ?? config.cs2Path;

  const applyPath = async () => {
    await patchConfig("cs2Path", path.trim());
    setPathDraft(null);
  };

  const detectPath = async () => {
    // The auto-detect logic lives in CfgManager and emits its own status
    // events; here we only need to refresh the displayed value after.
    try {
      // No dedicated command exists yet, but the UI can prompt the user
      // to paste the path manually. Reserved for a future command.
      console.warn("auto-detect not wired up yet");
    } catch (err) {
      console.error(err);
    }
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
        <div className="mt-3">
          <Toggle
            checked={config.preferTeamChat}
            onChange={(v) => patchConfig("preferTeamChat", v)}
            label={t("cfg.prefer_team_chat")}
          />
        </div>
      </Card>

      <Card title={t("cfg.global_keys")}>
        <KeyEditor
          kind="global"
          sample="k"
          summaryKey="cfg.current_global_keys"
        />
      </Card>
      <Card title={t("cfg.team_keys")}>
        <KeyEditor kind="team" sample="l" summaryKey="cfg.current_team_keys" />
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
