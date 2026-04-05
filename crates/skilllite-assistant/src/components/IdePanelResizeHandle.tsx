import { useCallback } from "react";

type Props = {
  ariaLabel: string;
  /** 向右拖增加该侧面板宽度（左侧栏右缘） */
  direction: 1 | -1;
  getStartWidth: () => number;
  clamp: (w: number) => number;
  onDrag: (w: number) => void;
  onCommit: (w: number) => void;
};

/**
 * IDE 三栏之间的纵向分隔条（pointer 拖拽调宽）。
 */
export default function IdePanelResizeHandle({
  ariaLabel,
  direction,
  getStartWidth,
  clamp,
  onDrag,
  onCommit,
}: Props) {
  const onPointerDown = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      const target = e.currentTarget;
      target.setPointerCapture(e.pointerId);
      const startX = e.clientX;
      const startW = getStartWidth();

      const handleMove = (ev: PointerEvent) => {
        const dx = ev.clientX - startX;
        onDrag(clamp(startW + direction * dx));
      };

      const finish = (ev: PointerEvent) => {
        if (target.hasPointerCapture(ev.pointerId)) {
          target.releasePointerCapture(ev.pointerId);
        }
        document.removeEventListener("pointermove", handleMove);
        document.removeEventListener("pointerup", finish);
        document.removeEventListener("pointercancel", finish);
        document.body.style.removeProperty("cursor");
        document.body.style.removeProperty("user-select");
        const dx = ev.clientX - startX;
        onCommit(clamp(startW + direction * dx));
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("pointermove", handleMove);
      document.addEventListener("pointerup", finish);
      document.addEventListener("pointercancel", finish);
    },
    [clamp, direction, getStartWidth, onDrag, onCommit]
  );

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={ariaLabel}
      onPointerDown={onPointerDown}
      className="w-3 shrink-0 z-10 -mx-1 cursor-col-resize hover:bg-accent/10 active:bg-accent/20 select-none touch-none"
    />
  );
}
