import { type HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

/**
 * Hairline horizontal divider — 1px, low-alpha white. Matches the Fluent
 * `divider` token. Vertical variant via `vertical` prop.
 */
export function Divider({
  vertical,
  className,
  ...rest
}: HTMLAttributes<HTMLDivElement> & { vertical?: boolean }) {
  return (
    <div
      role="separator"
      className={cn(
        vertical
          ? "w-px h-full bg-stroke-divider"
          : "h-px w-full bg-stroke-divider",
        className,
      )}
      {...rest}
    />
  );
}
