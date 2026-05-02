import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export interface ModeOptionProps {
  selected: boolean;
  label: string;
  description?: ReactNode;
  onSelect(): void;
}

/**
 * Radio-button card used by mode pickers (CFG dispatch channel, CFG vs.
 * chat trigger, …). Pure presentational — caller owns the `selected`
 * state.
 */
export function ModeOption({ selected, label, description, onSelect }: ModeOptionProps) {
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
      {description ? (
        <p className="mt-1.5 text-[12px] text-fg-tertiary leading-snug">{description}</p>
      ) : null}
    </button>
  );
}
