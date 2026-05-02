import { type ButtonHTMLAttributes, forwardRef } from "react";
import { cn } from "@/lib/cn";

type Variant = "accent" | "standard" | "subtle" | "danger";
type Size = "sm" | "md";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
}

const VARIANT: Record<Variant, string> = {
  accent:
    "bg-accent-base text-fg-on-accent border-transparent " +
    "hover:bg-accent-hover active:bg-accent-pressed " +
    "disabled:bg-accent-disabled disabled:text-fg-disabled",
  standard:
    "bg-fill-control text-fg-primary border-stroke-control " +
    "hover:bg-fill-control-hover active:bg-fill-control-pressed " +
    "disabled:bg-fill-control-disabled disabled:text-fg-disabled disabled:border-transparent",
  subtle:
    "bg-fill-subtle text-fg-primary border-transparent " +
    "hover:bg-fill-subtle-hover active:bg-fill-subtle-pressed " +
    "disabled:text-fg-disabled",
  danger:
    "bg-fill-control text-error border-stroke-control " +
    "hover:bg-fill-control-hover hover:text-error active:bg-fill-control-pressed " +
    "disabled:bg-fill-control-disabled disabled:text-fg-disabled",
};

const SIZE: Record<Size, string> = {
  sm: "h-7 px-3 text-[12px] gap-1.5 rounded-sm",
  md: "h-8 px-4 text-[13px] gap-2 rounded-sm",
};

/**
 * Win11 Fluent button. Three semantic variants:
 *  - `accent`    : primary CTA (Win11 accent ramp).
 *  - `standard`  : default secondary control.
 *  - `subtle`    : transparent until hovered — for low-emphasis actions
 *                   embedded inside a card.
 *  - `danger`    : same shape as `standard` but with `error` foreground.
 *
 * Reveal hover (Win11 mouse-tracking highlight) is opt-in via
 * `data-reveal` set on every variant.
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant = "standard", size = "md", children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      data-reveal
      className={cn(
        "inline-flex items-center justify-center whitespace-nowrap",
        "border font-medium select-none cursor-default",
        "transition-[background,color,border,box-shadow]",
        "duration-(--duration-fast) ease-(--ease-fluent)",
        "disabled:cursor-not-allowed",
        SIZE[size],
        VARIANT[variant],
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
});
