import { useState, useEffect, useRef } from "react";
import type { ModelPreset } from "../utils/modelPresets";

export default function ModelComboBox({
  value,
  onChange,
  onPresetSelect,
  presets,
  placeholder,
  inputCls,
}: {
  value: string;
  onChange: (v: string) => void;
  onPresetSelect?: (preset: ModelPreset) => void;
  presets: ModelPreset[];
  placeholder: string;
  inputCls: string;
}) {
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [customMode, setCustomMode] = useState(
    () => !presets.some((p) => p.value === value) && value !== ""
  );
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!dropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [dropdownOpen]);

  const matched = presets.find((p) => p.value === value);

  if (customMode) {
    return (
      <div className="flex gap-2">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={`flex-1 min-w-0 ${inputCls}`}
        />
        <button
          type="button"
          onClick={() => {
            setCustomMode(false);
            const first = presets[0];
            if (first && !presets.some((p) => p.value === value)) {
              onChange(first.value);
              onPresetSelect?.(first);
            }
          }}
          className="shrink-0 px-2.5 py-2 rounded-lg border border-border dark:border-border-dark text-ink-mute dark:text-ink-dark-mute hover:bg-gray-100 dark:hover:bg-white/5 text-xs font-medium transition-colors"
        >
          预设
        </button>
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setDropdownOpen(!dropdownOpen)}
        className={`${inputCls} text-left flex items-center justify-between gap-2 cursor-pointer`}
      >
        <span className={matched ? "text-ink dark:text-ink-dark" : "text-ink-mute"}>
          {matched ? matched.label : placeholder}
        </span>
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="shrink-0 text-ink-mute"
        >
          <path d="m6 9 6 6 6-6" />
        </svg>
      </button>
      {dropdownOpen && (
        <div className="absolute z-10 mt-1 w-full rounded-lg border border-border dark:border-border-dark bg-white dark:bg-paper-dark shadow-lg max-h-48 overflow-y-auto">
          {presets.map((p) => (
            <button
              key={p.value}
              type="button"
              onClick={() => {
                // Preset first so parents can sync apiBase refs before model onChange runs.
                onPresetSelect?.(p);
                onChange(p.value);
                setDropdownOpen(false);
              }}
              className={`w-full text-left px-3 py-2 text-sm transition-colors ${
                value === p.value
                  ? "bg-accent/10 text-accent font-medium"
                  : "text-ink dark:text-ink-dark hover:bg-gray-50 dark:hover:bg-white/5"
              }`}
            >
              <span>{p.label}</span>
              <span className="text-xs text-ink-mute dark:text-ink-dark-mute ml-2">{p.value}</span>
            </button>
          ))}
          <button
            type="button"
            onClick={() => {
              setCustomMode(true);
              setDropdownOpen(false);
            }}
            className="w-full text-left px-3 py-2 text-sm text-accent hover:bg-gray-50 dark:hover:bg-white/5 border-t border-border dark:border-border-dark"
          >
            自定义输入…
          </button>
        </div>
      )}
    </div>
  );
}
