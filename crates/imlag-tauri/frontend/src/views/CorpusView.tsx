import { useState } from "react";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { Button, Card, Input, ListItem } from "@/components";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";
import { api } from "@/lib/api";

export function CorpusView() {
  const { corpus } = useEngine();
  const { t } = useT();
  const [draft, setDraft] = useState("");

  const add = async () => {
    const v = draft.trim();
    if (!v) return;
    await api.corpusAdd(v);
    setDraft("");
  };

  const remove = async (msg: string) => {
    await api.corpusRemove(msg);
  };

  const onImport = async () => {
    try {
      const picked = await openFileDialog({
        title: t("corpus.dialog.import_title"),
        multiple: false,
        directory: false,
        filters: [
          { name: "Text", extensions: ["txt", "json"] },
          { name: "All", extensions: ["*"] },
        ],
      });
      if (typeof picked !== "string") return;
      await api.corpusImport(picked);
    } catch (err) {
      console.error("[corpus] import failed", err);
    }
  };

  const onExport = async () => {
    try {
      const target = await saveFileDialog({
        title: t("corpus.dialog.export_title"),
        defaultPath: t("corpus.dialog.default_name"),
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (typeof target !== "string") return;
      await api.corpusExport(target);
    } catch (err) {
      console.error("[corpus] export failed", err);
    }
  };

  return (
    <div className="fluent-enter flex flex-col gap-4">
      <header>
        <h2 className="text-[20px] font-semibold text-fg-primary">
          {t("section.corpus")}
        </h2>
        <p className="text-[12px] text-fg-tertiary">
          {corpus.length} entries · 死亡时随机挑一句发送
        </p>
      </header>

      <Card>
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                add();
              }
            }}
            placeholder={t("corpus.placeholder")}
            className="flex-1"
          />
          <Button variant="accent" onClick={add}>
            {t("common.add")}
          </Button>
          <Button variant="standard" onClick={onImport}>
            {t("corpus.import_file")}
          </Button>
          <Button variant="standard" onClick={onExport}>
            {t("corpus.export_file")}
          </Button>
        </div>
      </Card>

      <Card dense>
        {corpus.length === 0 ? (
          <p className="px-2 py-4 text-center text-[12px] text-fg-tertiary">
            {t("corpus.empty")}
          </p>
        ) : (
          <ul className="flex max-h-[420px] flex-col gap-0.5 overflow-y-auto pr-1">
            {corpus.map((msg, idx) => (
              <ListItem
                key={`${idx}:${msg}`}
                primary={msg}
                onRemove={() => remove(msg)}
              />
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
