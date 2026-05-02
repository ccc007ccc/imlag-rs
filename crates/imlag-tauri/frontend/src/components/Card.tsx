import { type HTMLAttributes, type ReactNode } from "react";
import { cn } from "@/lib/cn";

interface CardProps extends Omit<HTMLAttributes<HTMLDivElement>, "title"> {
  /** Optional title rendered above the body with a thin divider. */
  title?: ReactNode;
  /** Optional subtitle / hint rendered next to the title. */
  hint?: ReactNode;
  /** Tighter padding for dense lists. */
  dense?: boolean;
}

/**
 * Fluent surface — translucent fill above the acrylic backdrop, hairline
 * border, depth-2 shadow. The de-facto container for every settings
 * group in the app.
 */
export function Card({
  title,
  hint,
  dense,
  className,
  children,
  ...rest
}: CardProps) {
  return (
    <section
      className={cn(
        "rounded-lg border border-stroke-default bg-fill-card",
        "shadow-fluent-2 backdrop-blur-[2px]",
        dense ? "p-3" : "p-4",
        className,
      )}
      {...rest}
    >
      {(title || hint) && (
        <header className="mb-3 flex items-baseline gap-2">
          {title && (
            <h3 className="text-[14px] font-semibold text-fg-primary">
              {title}
            </h3>
          )}
          {hint && (
            <span className="text-[12px] text-fg-tertiary">{hint}</span>
          )}
        </header>
      )}
      {children}
    </section>
  );
}
