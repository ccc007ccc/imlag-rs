// Win11 reveal-hover initialiser. Attaches a single document-level
// pointermove listener that writes `--reveal-x` / `--reveal-y` on the
// closest ancestor element with `data-reveal`. Components opt-in by
// adding the attribute; no per-instance bookkeeping required.
export function installReveal(): () => void {
  const onMove = (e: PointerEvent) => {
    const target = (e.target as HTMLElement | null)?.closest<HTMLElement>(
      "[data-reveal]",
    );
    if (!target) return;
    const rect = target.getBoundingClientRect();
    target.style.setProperty("--reveal-x", `${e.clientX - rect.left}px`);
    target.style.setProperty("--reveal-y", `${e.clientY - rect.top}px`);
  };
  document.addEventListener("pointermove", onMove, { passive: true });
  return () => document.removeEventListener("pointermove", onMove);
}
