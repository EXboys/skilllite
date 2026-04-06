import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";

interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop?: () => void;
  disabled: boolean;
  loading: boolean;
  placeholder?: string;
  /** 输入框上方：附图、工具条等 */
  attachmentSlot?: ReactNode;
  /** 允许在正文为空时发送（例如仅发图片） */
  allowEmptySend?: boolean;
  /** 仅渲染输入行，用于与上方模块同处一个底栏容器 */
  bare?: boolean;
  /** 输入框下方的附加行（如选项开关） */
  footer?: ReactNode;
}

export function ChatInput({
  value,
  onChange,
  onSend,
  onStop,
  disabled,
  loading,
  placeholder = "Enter to send · Shift+Enter for newline",
  attachmentSlot,
  allowEmptySend = false,
  bare = false,
  footer,
}: ChatInputProps) {
  /** True while IME composition is active, or until deferred clear after compositionend (WebKit ordering). */
  const imeComposingRef = useRef(false);
  const imeEndClearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (imeEndClearTimerRef.current !== null) {
        clearTimeout(imeEndClearTimerRef.current);
      }
    };
  }, []);

  const cancelDeferredImeEnd = () => {
    if (imeEndClearTimerRef.current !== null) {
      clearTimeout(imeEndClearTimerRef.current);
      imeEndClearTimerRef.current = null;
    }
  };

  const handleCompositionStart = () => {
    cancelDeferredImeEnd();
    imeComposingRef.current = true;
  };

  const handleCompositionEnd = () => {
    cancelDeferredImeEnd();
    // WebKit/WKWebView often fires compositionend before the Enter keydown that commits text.
    // Clearing synchronously makes that keydown look "not composing" and preventDefault breaks IME.
    imeEndClearTimerRef.current = setTimeout(() => {
      imeComposingRef.current = false;
      imeEndClearTimerRef.current = null;
    }, 0);
  };

  /** Enter belongs to IME (candidate confirm, etc.) — do not send. */
  const isImeConsumingEnter = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    const ne = e.nativeEvent;
    if (ne.isComposing || imeComposingRef.current) return true;
    // Chromium legacy: keyCode 229 while IME is handling the key.
    if (ne.keyCode === 229) return true;
    if (e.key === "Process") return true;
    return false;
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== "Enter" || e.shiftKey) return;
    if (disabled || loading) return;
    if (isImeConsumingEnter(e)) return;
    e.preventDefault();
    if (!value.trim() && !allowEmptySend) return;
    onSend();
  };

  const row = (
    <div className="flex gap-2">
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={handleKeyDown}
          onCompositionStart={handleCompositionStart}
          onCompositionEnd={handleCompositionEnd}
          placeholder={placeholder}
          disabled={disabled}
          rows={3}
          className="flex-1 rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-4 py-2.5 text-ink dark:text-ink-dark placeholder-ink-mute dark:placeholder-ink-dark-mute focus:ring-2 focus:ring-accent/30 focus:border-accent outline-none disabled:opacity-50 resize-y min-h-[44px] max-h-52"
        />
        {loading && onStop ? (
          <button
            type="button"
            onClick={onStop}
            className="px-4 py-2.5 rounded-lg border border-red-300 dark:border-red-700 text-red-600 dark:text-red-400 text-sm font-medium hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
          >
            停止
          </button>
        ) : (
          <button
            type="button"
            onClick={onSend}
            disabled={
              disabled ||
              loading ||
              (!value.trim() && !allowEmptySend)
            }
            className="px-4 py-2.5 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            发送
          </button>
        )}
      </div>
  );

  const body = (
    <div className={`flex flex-col gap-2 ${bare ? "w-full" : ""}`}>
      {attachmentSlot}
      {row}
      {footer}
    </div>
  );

  if (bare) return body;

  return (
    <div className="p-4 border-t border-border dark:border-border-dark bg-white dark:bg-paper-dark">
      {body}
    </div>
  );
}
