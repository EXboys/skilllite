interface ChatInputProps {
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onStop?: () => void;
  disabled: boolean;
  loading: boolean;
  placeholder?: string;
  /** 仅渲染输入行，用于与上方模块同处一个底栏容器 */
  bare?: boolean;
}

export function ChatInput({
  value,
  onChange,
  onSend,
  onStop,
  disabled,
  loading,
  placeholder = "输入指令（Enter 换行，点击发送）…",
  bare = false,
}: ChatInputProps) {
  const row = (
    <div className="flex gap-2">
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
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
            disabled={disabled || !value.trim()}
            className="px-4 py-2.5 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            发送
          </button>
        )}
      </div>
  );

  if (bare) return row;

  return (
    <div className="p-4 border-t border-border dark:border-border-dark bg-white dark:bg-paper-dark">
      {row}
    </div>
  );
}
