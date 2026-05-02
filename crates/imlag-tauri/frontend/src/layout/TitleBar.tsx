import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { cn } from "@/lib/cn";
import { useT, LANG_CYCLE } from "@/lib/i18n";
import type { LangCode } from "@/lib/types";
import logoUrl from "@/assets/logo.png";

interface CaptionButtonProps {
  onClick(): void;
  label: string;
  className?: string;
  children: React.ReactNode;
}

function CaptionButton({ onClick, label, className, children }: CaptionButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "inline-flex h-full w-12 items-center justify-center",
        "text-fg-secondary",
        "transition-colors duration-(--duration-fast) ease-(--ease-fluent)",
        className,
      )}
    >
      {children}
    </button>
  );
}

const Glyph = {
  Min: () => (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <rect x="0" y="4.5" width="10" height="1" fill="currentColor" />
    </svg>
  ),
  Max: ({ maximized }: { maximized: boolean }) => (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      {maximized ? (
        // Restore 图标：两个交叠的描边方框，前景框用 path 绕开后景框区域，
        // 不需要任何实色填充，避免出现"中间黑色方块"。
        <>
          <path
            d="M2.5 0.5 H9.5 V7.5 H7.5 V2.5 H2.5 Z"
            fill="none"
            stroke="currentColor"
            strokeLinejoin="miter"
          />
          <rect
            x="0.5"
            y="2.5"
            width="7"
            height="7"
            fill="none"
            stroke="currentColor"
          />
        </>
      ) : (
        <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" />
      )}
    </svg>
  ),
  Close: () => (
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
      <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" />
      <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" />
    </svg>
  ),
  // 地球图标 — 简化的经线/纬线网格，14px 适配 32px 标题栏。
  Globe: () => (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <circle cx="8" cy="8" r="6.4" />
      <ellipse cx="8" cy="8" rx="3" ry="6.4" />
      <line x1="1.6" y1="8" x2="14.4" y2="8" />
      <path d="M2.6 4.4 H13.4" />
      <path d="M2.6 11.6 H13.4" />
    </svg>
  ),
  Check: () => (
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden fill="none">
      <path
        d="M2.5 6.2 L4.8 8.5 L9.5 3.5"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  ),
};

/**
 * Custom title bar — 因为窗口配置 `decorations: false`，需要自绘。
 * 32 px 高（Win11 标准）。拖拽区通过 `data-tauri-drag-region` 声明，
 * caption 按钮加 `relative` 定位避免拖拽属性透传。
 *
 * 关闭按钮 hover 时变红（Win11 唯一的颜色例外）；最小化/最大化用
 * `fill-subtle-hover`；语言按钮用地球图标 + 下拉菜单。
 */
export function TitleBar() {
  const { lang, setLanguage, t } = useT();
  const [maximized, setMaximized] = useState(false);
  const [langOpen, setLangOpen] = useState(false);
  const langWrapRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const w = getCurrentWindow();
    let live = true;
    const probe = async () => {
      try {
        const m = await w.isMaximized();
        if (live) setMaximized(m);
      } catch {
        /* ignore */
      }
    };
    probe();
    const unsubP = w.onResized(() => probe());
    return () => {
      live = false;
      unsubP.then((u) => u()).catch(() => undefined);
    };
  }, []);

  // 点击下拉菜单外部 / 按 Esc 时关闭。
  useEffect(() => {
    if (!langOpen) return;
    const onPointer = (e: PointerEvent) => {
      const root = langWrapRef.current;
      if (!root) return;
      if (e.target instanceof Node && !root.contains(e.target)) {
        setLangOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setLangOpen(false);
    };
    window.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("keydown", onKey);
    };
  }, [langOpen]);

  const win = getCurrentWindow();
  const onMin = () => win.minimize().catch(console.warn);
  const onMax = () => win.toggleMaximize().catch(console.warn);
  const onClose = () => win.close().catch(console.warn);

  const onPickLang = (code: LangCode) => {
    setLangOpen(false);
    if (code === lang) return;
    setLanguage(code).catch(console.warn);
  };

  return (
    <header
      data-tauri-drag-region
      className={cn(
        "relative flex h-8 shrink-0 items-center select-none",
        "bg-fill-chrome",
        "border-b border-stroke-divider",
      )}
    >
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 px-3 text-[12px]"
      >
        <img
          src={logoUrl}
          alt=""
          aria-hidden
          className="h-4 w-4 rounded-[3px]"
          draggable={false}
        />
        <span className="font-semibold text-fg-primary tracking-tight">
          ImLag
        </span>
        <span className="text-fg-disabled">·</span>
        <span className="text-fg-tertiary">CS2 GSI Companion</span>
      </div>

      <div className="flex-1" data-tauri-drag-region />

      {/* 语言选择 — 地球图标 + 下拉菜单 */}
      <div ref={langWrapRef} className="relative h-full">
        <button
          type="button"
          aria-label={t("language.label")}
          aria-haspopup="listbox"
          aria-expanded={langOpen}
          onClick={() => setLangOpen((v) => !v)}
          className={cn(
            "inline-flex h-full w-10 items-center justify-center",
            "text-fg-secondary",
            "hover:bg-fill-subtle-hover hover:text-fg-primary",
            "transition-colors duration-(--duration-fast) ease-(--ease-fluent)",
            langOpen && "bg-fill-subtle-hover text-fg-primary",
          )}
          title={t("language.label")}
        >
          <Glyph.Globe />
        </button>
        {langOpen && (
          <ul
            role="listbox"
            aria-label={t("language.label")}
            className={cn(
              "absolute right-0 top-full z-50 mt-1 min-w-[140px]",
              "rounded-md border border-stroke-default bg-fill-card",
              "shadow-fluent-8 backdrop-blur-[2px]",
              "py-1 text-[12px]",
              "fluent-enter",
            )}
          >
            {LANG_CYCLE.map((code) => {
              const selected = code === lang;
              return (
                <li key={code}>
                  <button
                    type="button"
                    role="option"
                    aria-selected={selected}
                    onClick={() => onPickLang(code)}
                    className={cn(
                      "flex w-full items-center justify-between gap-3",
                      "px-3 py-1.5 text-left",
                      "transition-colors duration-(--duration-fast) ease-(--ease-fluent)",
                      selected
                        ? "text-fg-primary bg-accent-tertiary"
                        : "text-fg-secondary hover:bg-fill-subtle-hover hover:text-fg-primary",
                    )}
                  >
                    <span>{t(`language.${code}`)}</span>
                    <span
                      className={cn(
                        "text-fg-accent",
                        selected ? "opacity-100" : "opacity-0",
                      )}
                      aria-hidden
                    >
                      <Glyph.Check />
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <CaptionButton
        onClick={onMin}
        label="Minimize"
        className="hover:bg-fill-subtle-hover hover:text-fg-primary"
      >
        <Glyph.Min />
      </CaptionButton>
      <CaptionButton
        onClick={onMax}
        label={maximized ? "Restore" : "Maximize"}
        className="hover:bg-fill-subtle-hover hover:text-fg-primary"
      >
        <Glyph.Max maximized={maximized} />
      </CaptionButton>
      <CaptionButton
        onClick={onClose}
        label="Close"
        className="hover:bg-[#C42B1C] hover:text-white"
      >
        <Glyph.Close />
      </CaptionButton>
    </header>
  );
}
