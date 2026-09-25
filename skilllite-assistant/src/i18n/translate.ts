import { useSettingsStore } from "../stores/useSettingsStore";
import { zhMessages } from "./messages/zh";
import { enMessages } from "./messages/en";

export type Locale = "zh" | "en";

const bundles: Record<Locale, Record<string, string>> = {
  zh: zhMessages,
  en: enMessages,
};

function interpolate(
  template: string,
  vars?: Record<string, string | number>
): string {
  if (!vars) return template;
  let s = template;
  for (const [k, v] of Object.entries(vars)) {
    const needle = `{${k}}`;
    s = s.split(needle).join(String(v));
  }
  return s;
}

export function getLocale(): Locale {
  const l = useSettingsStore.getState().settings.locale;
  return l === "en" ? "en" : "zh";
}

/** 非 React 上下文（Toast、全局监听等）使用当前 store 语言。 */
export function translate(
  key: string,
  vars?: Record<string, string | number>,
  localeOverride?: Locale
): string {
  const loc = localeOverride ?? getLocale();
  const table = bundles[loc] ?? bundles.zh;
  const raw = table[key] ?? bundles.zh[key] ?? key;
  return interpolate(raw, vars);
}
