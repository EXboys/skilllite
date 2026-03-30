import { useEffect } from "react";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useI18n } from "../i18n";

const AUTO_DISMISS_MS = 5200;

export function ToastHost() {
  const { t } = useI18n();
  const message = useUiToastStore((s) => s.message);
  const variant = useUiToastStore((s) => s.variant);
  const clear = useUiToastStore((s) => s.clear);

  useEffect(() => {
    if (!message) return;
    const t = window.setTimeout(() => clear(), AUTO_DISMISS_MS);
    return () => window.clearTimeout(t);
  }, [message, clear]);

  if (!message) return null;

  const styles =
    variant === "error"
      ? "bg-red-900/95 text-red-50 border-red-700/80"
      : "bg-ink/90 dark:bg-paper-dark text-white dark:text-ink-dark border-white/10";

  return (
    <div
      className="fixed bottom-6 left-1/2 z-[100] max-w-[min(420px,calc(100vw-2rem))] -translate-x-1/2 px-4"
      role="status"
    >
      <div
        className={`rounded-lg border px-3.5 py-2.5 text-sm shadow-lg backdrop-blur-sm ${styles}`}
      >
        <p className="whitespace-pre-wrap break-words leading-snug">{message}</p>
        <button
          type="button"
          onClick={clear}
          className="mt-2 text-xs opacity-80 hover:opacity-100 underline"
        >
          {t("common.close")}
        </button>
      </div>
    </div>
  );
}
