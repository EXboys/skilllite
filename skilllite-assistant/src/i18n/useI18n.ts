import { useCallback, useEffect } from "react";
import { useSettingsStore } from "../stores/useSettingsStore";
import { translate as translateRaw, type Locale } from "./translate";

export function useI18n() {
  const locale = useSettingsStore((s) =>
    s.settings.locale === "en" ? "en" : "zh"
  );
  const setSettings = useSettingsStore((s) => s.setSettings);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) =>
      translateRaw(key, vars, locale),
    [locale]
  );

  useEffect(() => {
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  }, [locale]);

  const setLocale = useCallback(
    (l: Locale) => setSettings({ locale: l }),
    [setSettings]
  );

  return { t, locale, setLocale };
}
