import { useId, type ReactNode } from "react";
import { cn } from "@/lib/cn";

interface ToggleProps {
  checked: boolean;
  onChange(next: boolean): void;
  label?: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
  className?: string;
}

/**
 * Win11 toggle switch — CS2 配色版。40×20 轨道 + 圆形 thumb，
 * thumb 在 hover/press 时从 12 → 14px 放大，沿用 Fluent 的"按压"感。
 *
 * OFF 状态使用 `fill-track-off`（不透明深色）+ 强 stroke，
 * 避免在半透明卡片上"轨道几乎不可见"的问题。
 *
 * 视觉层面之下复用一个隐藏的原生 checkbox，键盘焦点 / Space 切换 /
 * 焦点描边都自动可用。
 */
export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
  className,
}: ToggleProps) {
  const id = useId();
  return (
    <label
      htmlFor={id}
      className={cn(
        "inline-flex items-center gap-3 select-none",
        disabled ? "cursor-not-allowed opacity-60" : "cursor-default",
        className,
      )}
    >
      <span className="relative inline-flex h-5 w-10 shrink-0 items-center">
        <input
          id={id}
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(e) => onChange(e.target.checked)}
          className="peer absolute inset-0 z-10 cursor-default opacity-0"
        />
        <span
          className={cn(
            "absolute inset-0 rounded-full border transition-colors",
            "duration-(--duration-normal) ease-(--ease-fluent)",
            checked
              ? "bg-accent-base border-transparent peer-hover:bg-accent-hover"
              : "bg-fill-track-off border-stroke-control-strong peer-hover:bg-fill-track-off-hover",
          )}
        />
        <span
          className={cn(
            "relative ml-0.5 h-3 w-3 rounded-full transition-all",
            "duration-(--duration-normal) ease-(--ease-fluent)",
            "peer-hover:h-3.5 peer-hover:w-3.5 peer-hover:ml-px",
            checked
              ? "translate-x-[22px] bg-fg-on-accent"
              : "translate-x-0 bg-fg-primary",
          )}
          style={{ pointerEvents: "none" }}
        />
      </span>
      {(label || hint) && (
        <span className="flex flex-col">
          {label && (
            <span className="text-[13px] text-fg-primary leading-tight">
              {label}
            </span>
          )}
          {hint && (
            <span className="text-[12px] text-fg-secondary">{hint}</span>
          )}
        </span>
      )}
    </label>
  );
}
