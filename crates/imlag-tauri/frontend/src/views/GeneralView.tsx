import { useState } from "react";
import { Button, Card, Input, ListItem, ModeOption, Toggle } from "@/components";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";
import { api } from "@/lib/api";

export function GeneralView() {
  const { config, gsiRunning, patchConfig } = useEngine();
  const { t } = useT();
  const [draft, setDraft] = useState("");

  if (!config) return null;

  const addPlayer = async () => {
    const trimmed = draft.trim();
    if (!trimmed) return;
    if (
      config.playerNames.some(
        (n) => n.toLowerCase() === trimmed.toLowerCase(),
      )
    ) {
      setDraft("");
      return;
    }
    await patchConfig("playerNames", [...config.playerNames, trimmed]);
    setDraft("");
  };

  const removePlayer = async (name: string) => {
    await patchConfig(
      "playerNames",
      config.playerNames.filter((n) => n !== name),
    );
  };

  const toggleGsi = async () => {
    try {
      if (gsiRunning) await api.stopGsi();
      else await api.startGsi();
    } catch (err) {
      console.error("[gsi] toggle failed", err);
    }
  };

  return (
    <div className="fluent-enter flex flex-col gap-4">
      <header>
        <h2 className="text-[20px] font-semibold text-fg-primary">
          {t("general.title")}
        </h2>
        <p className="mt-1 text-[12px] text-fg-secondary">{t("general.hint")}</p>
      </header>

      <Card>
        <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
          <Toggle
            checked={config.autoStartGsi}
            onChange={(v) => patchConfig("autoStartGsi", v)}
            label={t("general.auto_start_gsi")}
          />
          <Toggle
            checked={config.onlySelfDeath}
            onChange={(v) => patchConfig("onlySelfDeath", v)}
            label={t("general.only_self_death")}
          />
          <div className="ml-auto">
            <Button variant={gsiRunning ? "danger" : "accent"} onClick={toggleGsi}>
              {gsiRunning ? t("general.stop_gsi") : t("general.start_gsi")}
            </Button>
          </div>
        </div>
      </Card>

      <Card title={t("general.input_mode")} hint={t("general.input_mode_hint")}>
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

      <Card
        title={t("general.player_list_summary", config.playerNames.length)}
      >
        <div className="flex gap-2">
          <Input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addPlayer();
              }
            }}
            placeholder={t("general.player_add_placeholder")}
          />
          <Button variant="accent" onClick={addPlayer}>
            {t("common.add")}
          </Button>
        </div>

        {config.playerNames.length === 0 ? (
          <p className="mt-3 text-[12px] text-fg-tertiary">
            {t("general.player_list_empty")}
          </p>
        ) : (
          <ul className="mt-3 flex flex-col gap-0.5">
            {config.playerNames.map((name) => (
              <ListItem
                key={name}
                primary={name}
                onRemove={() => removePlayer(name)}
              />
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
