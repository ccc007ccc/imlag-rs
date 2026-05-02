import { useEffect, useState } from "react";
import { cn } from "@/lib/cn";
import { onUiEvent } from "@/lib/events";
import type { UiEventDto } from "@/lib/types";

const TONE: Record<UiEventDto["level"], string> = {
  info: "border-stroke-strong bg-fill-card text-fg-primary",
  warn: "border-[color:var(--color-warning)]/45 bg-[color:var(--color-warning)]/10 text-warning",
  error: "border-[color:var(--color-error)]/45 bg-[color:var(--color-error)]/12 text-error",
};

interface QueuedToast extends UiEventDto {
  id: number;
}

let nextId = 1;

/**
 * Floating toast stack at the bottom-right corner. Subscribes to the
 * engine's `ui-event` channel; each event becomes a card that fades out
 * after ~4 seconds. Errors stick around longer (8s) so the user has
 * time to read them.
 *
 * Only `warn` / `error` are surfaced as toasts — `info` events live in
 * the persistent status bar instead, to avoid noise.
 */
export function ToastStack() {
  const [toasts, setToasts] = useState<QueuedToast[]>([]);

  useEffect(() => {
    const unsubP = onUiEvent((evt) => {
      if (evt.level === "info") return;
      const id = nextId++;
      setToasts((prev) => [...prev, { ...evt, id }]);
      const ttl = evt.level === "error" ? 8_000 : 4_000;
      window.setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, ttl);
    });
    return () => {
      unsubP.then((unsub) => unsub());
    };
  }, []);

  return (
    <div className="pointer-events-none fixed right-4 bottom-12 z-50 flex flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={cn(
            "pointer-events-auto fluent-enter min-w-[220px] max-w-[360px]",
            "rounded-md border px-3 py-2 text-[12px] shadow-fluent-16 backdrop-blur",
            TONE[toast.level],
          )}
        >
          {toast.message}
        </div>
      ))}
    </div>
  );
}
