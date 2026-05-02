import { type InputHTMLAttributes, forwardRef } from "react";
import { cn } from "@/lib/cn";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Render an inline error styling — does not change layout. */
  invalid?: boolean;
}

/**
 * Fluent input — half-transparent fill that snaps fully opaque when
 * focused, and an accent underline that grows from 1px to 2px on focus.
 *
 * The bottom-only accent stroke is the Win11 signature — we render it
 * with a `box-shadow inset` instead of a real `border-bottom` so the
 * border-radius stays uniform.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, invalid, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      className={cn(
        "h-8 w-full px-3 text-[13px] rounded-sm",
        "bg-fill-input text-fg-primary placeholder:text-fg-tertiary",
        "border border-stroke-control",
        "focus:bg-fill-input-focused focus:outline-none focus:border-stroke-control",
        "focus:shadow-[inset_0_-2px_0_0_var(--color-accent-base)]",
        "hover:bg-fill-control-hover",
        "disabled:bg-fill-control-disabled disabled:text-fg-disabled disabled:cursor-not-allowed",
        "transition-[background,box-shadow,border]",
        "duration-(--duration-fast) ease-(--ease-fluent)",
        invalid &&
          "border-error focus:shadow-[inset_0_-2px_0_0_var(--color-error)]",
        className,
      )}
      {...rest}
    />
  );
});
