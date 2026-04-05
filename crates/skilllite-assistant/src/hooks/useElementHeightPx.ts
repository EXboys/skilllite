import { useLayoutEffect, useRef, useState } from "react";

/**
 * Observed pixel height for flex/portal children where `height: 100%` stays 0 in some WebViews
 * (e.g. Tauri WKWebView) until an explicit size is passed to CodeMirror.
 */
export function useElementHeightPx(active: boolean, minPx = 160) {
  const ref = useRef<HTMLDivElement>(null);
  const [heightPx, setHeightPx] = useState(minPx);

  useLayoutEffect(() => {
    if (!active) return;
    const el = ref.current;
    if (!el) return;

    const measure = () => {
      const next = el.getBoundingClientRect().height;
      if (next >= 1) {
        setHeightPx(Math.max(minPx, Math.floor(next)));
      }
    };

    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [active, minPx]);

  return { ref, heightPx };
}
