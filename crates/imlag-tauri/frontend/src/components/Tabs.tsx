import { type ReactNode } from "react";
import { cn } from "@/lib/cn";

interface TabItem<T extends string> {
  id: T;
  label: ReactNode;
}

interface TabsProps<T extends string> {
  items: ReadonlyArray<TabItem<T>>;
  active: T;
  onChange(id: T): void;
  className?: string;
}

/**
 * Win11 navigation tabs — selection sits beneath the row as a 2px accent
 * pill that animates between tabs. The motion is implicit because the
 * accent bar is rendered per-item; CSS `view-transition` would be nicer
 * but isn't available in webview2 yet.
 */
export function Tabs<T extends string>({
  items,
  active,
  onChange,
  className,
}: TabsProps<T>) {
  return (
    <nav
      className={cn(
        "flex items-end gap-1 border-b border-stroke-divider px-1",
        "bg-fill-chrome-soft",
        className,
      )}
    >
      {items.map(({ id, label }) => {
        const selected = id === active;
        return (
          <button
            key={id}
            data-reveal
            onClick={() => onChange(id)}
            className={cn(
              "relative px-4 h-9 inline-flex items-center text-[13px]",
              "rounded-md transition-colors",
              "duration-(--duration-fast) ease-(--ease-fluent)",
              selected
                ? "text-fg-primary font-semibold"
                : "text-fg-secondary hover:bg-fill-subtle-hover hover:text-fg-primary",
            )}
          >
            {label}
            <span
              aria-hidden
              className={cn(
                "absolute left-3 right-3 -bottom-px h-0.5 rounded-full",
                "transition-opacity duration-(--duration-normal) ease-(--ease-fluent)",
                selected ? "bg-accent-base opacity-100" : "opacity-0",
              )}
            />
          </button>
        );
      })}
    </nav>
  );
}
