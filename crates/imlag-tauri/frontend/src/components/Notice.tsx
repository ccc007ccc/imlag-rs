import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export type NoticeTone = "info" | "warning";

export interface NoticeProps {
  tone?: NoticeTone;
  children: ReactNode;
  className?: string;
}

/**
 * Inline status banner. Less prominent than a `Card`, more permanent than
 * a toast — used for "this view is currently inactive" hints and similar
 * passive guidance.
 */
export function Notice({ tone = "info", children, className }: NoticeProps) {
  const palette =
    tone === "warning"
      ? "border-warning/40 bg-warning/10 text-warning"
      : "border-stroke-default bg-fill-control text-fg-secondary";
  return (
    <div
      role="note"
      data-reveal
      className={cn(
        "rounded-md border px-3 py-2 text-[12px] leading-snug",
        palette,
        className,
      )}
    >
      {children}
    </div>
  );
}
