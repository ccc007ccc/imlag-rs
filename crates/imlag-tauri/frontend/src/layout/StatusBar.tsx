import { useEffect, useState } from "react";
import { cn } from "@/lib/cn";
import { onUiEvent } from "@/lib/events";
import { useEngine } from "@/lib/engine";
import { useT } from "@/lib/i18n";
import type { UiEventDto } from "@/lib/types";
import { StatusDot } from "@/components/Pill";

const TONE_FOR_LEVEL: Record<UiEventDto["level"], string> = {
  info: "text-fg-secondary",
  warn: "text-warning",
  error: "text-error",
};

/**
 * Persistent status bar at the bottom edge — left side carries the GSI
 * state and the most recent UI event, right side surfaces aggregate
 * counters (corpus size, generated CFG groups).
 */
export function StatusBar() {
  const { gsiRunning, stats } = useEngine();
  const { t } = useT();
  const [lastEvent, setLastEvent] = useState<UiEventDto | null>(null);

  useEffect(() => {
    const unsubP = onUiEvent(setLastEvent);
    return () => {
      unsubP.then((u) => u()).catch(() => undefined);
    };
  }, []);

  return (
    <footer
      className={cn(
        "flex h-7 shrink-0 items-center gap-3 border-t border-stroke-divider",
        "bg-fill-chrome",
        "px-4 text-[11px] text-fg-tertiary",
      )}
    >
      <span className="flex items-center gap-1.5">
        <StatusDot tone={gsiRunning ? "success" : "neutral"} />
        <span className="text-fg-secondary">
          {gsiRunning ? t("gsi.started") : t("gsi.stopped")}
        </span>
      </span>

      <span aria-hidden className="text-fg-disabled">|</span>

      <span className={cn("min-w-0 flex-1 truncate", lastEvent ? TONE_FOR_LEVEL[lastEvent.level] : "")}>
        {lastEvent?.message ?? t("status.init")}
      </span>

      <span className="shrink-0 text-fg-tertiary">
        {t("stats.summary", stats.corpusCount, stats.cfgGroupCount).replace(/\n/g, " · ")}
      </span>
    </footer>
  );
}
