import { useEffect, useState } from "react";
import { Tabs, ToastStack } from "@/components";
import { TitleBar } from "@/layout/TitleBar";
import { StatusBar } from "@/layout/StatusBar";
import { EngineProvider } from "@/lib/engine";
import { installReveal } from "@/lib/reveal";
import { useT } from "@/lib/i18n";
import {
  CfgModeView,
  ChatModeView,
  CorpusView,
  GeneralView,
} from "@/views";
import type { Tab } from "@/lib/types";

function MainPanel() {
  const [tab, setTab] = useState<Tab>("general");
  const { t, lang } = useT();

  // Re-key the body when the language flips so the entrance animation
  // replays with the freshly translated copy — much nicer than seeing
  // mid-stream label swaps.
  const bodyKey = `${tab}:${lang}`;

  return (
    <main className="relative flex min-h-0 flex-1 flex-col">
      <Tabs<Tab>
        active={tab}
        onChange={setTab}
        className="px-4 pt-1"
        items={[
          { id: "general", label: t("section.general") },
          { id: "cfg", label: t("section.cfg") },
          { id: "chat", label: t("section.chat") },
          { id: "corpus", label: t("section.corpus") },
        ]}
      />
      <section
        key={bodyKey}
        className="min-h-0 flex-1 overflow-y-auto px-6 py-5"
      >
        {tab === "general" && <GeneralView />}
        {tab === "cfg" && <CfgModeView />}
        {tab === "chat" && <ChatModeView />}
        {tab === "corpus" && <CorpusView />}
      </section>
    </main>
  );
}

export function App() {
  // Reveal hover is a singleton — install once for the whole document.
  useEffect(() => installReveal(), []);

  return (
    <EngineProvider>
      <div className="flex h-full min-h-0 flex-col">
        <TitleBar />
        <MainPanel />
        <StatusBar />
        <ToastStack />
      </div>
    </EngineProvider>
  );
}
