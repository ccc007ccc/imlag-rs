import { type ReactNode } from "react";
import { cn } from "@/lib/cn";

type Tone = "neutral" | "accent" | "success" | "warning" | "error";

interface PillProps {
  tone?: Tone;
  className?: string;
  children: ReactNode;
}

const TONE: Record<Tone, string> = {
  neutral:
    "bg-fill-control text-fg-secondary border-stroke-control",
  accent:
    "bg-accent-tertiary text-fg-accent border-[color:var(--color-accent-base)]/40",
  success:
    "bg-[color:var(--color-success)]/15 text-success border-[color:var(--color-success)]/45",
  warning:
    "bg-[color:var(--color-warning)]/15 text-warning border-[color:var(--color-warning)]/45",
  error:
    "bg-[color:var(--color-error)]/15 text-error border-[color:var(--color-error)]/45",
};

/**
 * A pill-shaped status badge — used for "GSI Online", counters, or
 * inline category tags. 999px radius, hairline border, low-alpha fill.
 */
export function Pill({ tone = "neutral", className, children }: PillProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 px-2 h-5",
        "text-[11px] font-medium tracking-tight rounded-full border",
        TONE[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

/**
 * A small dot used as a status indicator — colour matches the tone.
 */
export function StatusDot({
  tone = "neutral",
  className,
}: {
  tone?: Tone;
  className?: string;
}) {
  const TONE_BG: Record<Tone, string> = {
    neutral: "bg-fg-tertiary",
    accent: "bg-accent-base",
    success: "bg-success",
    warning: "bg-warning",
    error: "bg-error",
  };
  return (
    <span
      className={cn("inline-block h-2 w-2 rounded-full", TONE_BG[tone], className)}
    />
  );
}
