import { type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Button } from "./Button";
import { useT } from "@/lib/i18n";

interface ListItemProps {
  primary: ReactNode;
  secondary?: ReactNode;
  /** Removal callback. When omitted no delete control is rendered. */
  onRemove?: () => void;
  className?: string;
}

/**
 * Reusable list row — primary text on the left, optional muted detail,
 * and an inline "delete" subtle button on the right. Hover is mostly
 * carried by the parent container; the row itself only adds a faint
 * fill so the action button stays visually anchored.
 */
export function ListItem({
  primary,
  secondary,
  onRemove,
  className,
}: ListItemProps) {
  const { t } = useT();
  return (
    <li
      className={cn(
        "group flex items-center gap-3 rounded-sm",
        "px-3 py-2 hover:bg-fill-subtle-hover",
        "transition-colors duration-(--duration-fast) ease-(--ease-fluent)",
        className,
      )}
    >
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[13px] text-fg-primary">{primary}</span>
        {secondary && (
          <span className="truncate text-[12px] text-fg-tertiary">
            {secondary}
          </span>
        )}
      </div>
      {onRemove && (
        <Button
          variant="subtle"
          size="sm"
          onClick={onRemove}
          className="opacity-0 group-hover:opacity-100"
        >
          {t("common.delete")}
        </Button>
      )}
    </li>
  );
}
