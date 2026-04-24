import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useUiToastStore } from "../stores/useUiToastStore";

const DEFAULT_BIND = "127.0.0.1:8787";

/** Parse `host:port` (IPv4-style); fallback for empty or malformed. */
function parseHostPort(bind: string): { host: string; port: string } {
  const s = bind.trim() || DEFAULT_BIND;
  const i = s.lastIndexOf(":");
  if (i <= 0 || i === s.length - 1) {
    return { host: "127.0.0.1", port: "8787" };
  }
  return { host: s.slice(0, i), port: s.slice(i + 1) };
}

/** Base URL for browser health check from this machine (`0.0.0.0` → `127.0.0.1`). */
function httpBaseForLocalFetch(bind: string): string {
  const { host, port } = parseHostPort(bind);
  const h = host === "0.0.0.0" ? "127.0.0.1" : host;
  return `http://${h}:${port}`;
}

function shellQuoteSingle(raw: string): string {
  return `'${raw.replace(/'/g, `'\"'\"'`)}'`;
}

function buildStartCommand(bind: string, token: string | undefined, artifactDir: string | undefined): string {
  const b = bind.trim() || DEFAULT_BIND;
  let cmd = `SKILLLITE_GATEWAY_SERVE_ALLOW=1 skilllite gateway serve --bind ${b}`;
  const t = token?.trim();
  if (t) {
    cmd += ` --token ${shellQuoteSingle(t)}`;
  }
  const a = artifactDir?.trim();
  if (a) {
    cmd += ` --artifact-dir ${shellQuoteSingle(a)}`;
  }
  return cmd;
}

/** Map common transport errors to a short, actionable message (locale via `t`). */
function humanizeGatewayHealthDetail(
  raw: string,
  t: (key: string, vars?: Record<string, string | number>) => string
): string {
  const lower = raw.toLowerCase();
  if (
    lower.includes("connection refused") ||
    (lower.includes("connect error") && lower.includes("refused")) ||
    lower.includes("os error 61") ||
    lower.includes("os error 111") ||
    lower.includes("os error 10061")
  ) {
    return t("settings.gatewayServe.healthRefusedHint");
  }
  if (lower.includes("timed out") || lower.includes("timeout")) {
    return t("settings.gatewayServe.healthTimeoutHint");
  }
  return raw;
}

export default function GatewayServeSettingsSection() {
  const { t } = useI18n();
  const { settings, setSettings } = useSettingsStore();
  const [healthBusy, setHealthBusy] = useState(false);
  const [healthLabel, setHealthLabel] = useState<"idle" | "ok" | "fail">("idle");

  const bind = settings.gatewayServeBind ?? DEFAULT_BIND;
  const token = settings.gatewayServeToken ?? "";
  const artifactDir = settings.gatewayArtifactDir ?? "";

  const baseUrl = useMemo(() => httpBaseForLocalFetch(bind), [bind]);
  const healthUrl = `${baseUrl}/health`;
  const webhookUrl = `${baseUrl}/webhook/inbound`;
  const artifactUrl = `${baseUrl}/v1/runs/<run_id>/artifacts?key=<key>`;
  const startCmd = useMemo(
    () => buildStartCommand(bind, token || undefined, artifactDir || undefined),
    [artifactDir, bind, token]
  );

  const copyText = useCallback(async (text: string, okMsg: string) => {
    try {
      await navigator.clipboard.writeText(text);
      useUiToastStore.getState().show(okMsg, "info");
    } catch {
      useUiToastStore.getState().show(t("settings.gatewayServe.clipboardFail"), "error");
    }
  }, [t]);

  const runHealthCheck = useCallback(async () => {
    setHealthBusy(true);
    setHealthLabel("idle");
    try {
      /** Native-side HTTP: WebView `fetch` to `http://127.0.0.1` often fails with "Load failed" (CORS / mixed content). */
      const r = await invoke<{ ok: boolean; status?: number; error?: string }>(
        "assistant_gateway_health_probe",
        { url: healthUrl }
      );
      if (r.ok) {
        setHealthLabel("ok");
        useUiToastStore.getState().show(t("settings.gatewayServe.healthOk"), "info");
      } else {
        setHealthLabel("fail");
        const raw =
          r.error ??
          (r.status !== undefined && r.status !== null
            ? t("settings.gatewayServe.healthFail", { status: String(r.status) })
            : "unknown");
        const detail = humanizeGatewayHealthDetail(raw, t);
        useUiToastStore.getState().show(t("settings.gatewayServe.healthError", { msg: detail }), "error");
      }
    } catch (e) {
      setHealthLabel("fail");
      const msg = humanizeGatewayHealthDetail(
        e instanceof Error ? e.message : String(e),
        t
      );
      useUiToastStore.getState().show(t("settings.gatewayServe.healthError", { msg }), "error");
    } finally {
      setHealthBusy(false);
    }
  }, [healthUrl, t]);

  return (
    <section
      className="overflow-hidden rounded-xl border border-border bg-white dark:border-border-dark dark:bg-paper-dark"
      aria-labelledby="gateway-serve-heading"
    >
      <div className="border-b border-border/80 bg-ink/[0.03] px-4 py-3 dark:border-border-dark/80 dark:bg-white/[0.04]">
        <h3 id="gateway-serve-heading" className="text-sm font-semibold text-ink dark:text-ink-dark">
          {t("settings.gatewayServe.title")}
        </h3>
        <p className="mt-1 text-xs leading-snug text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.subtitle")}</p>
      </div>

      <div className="space-y-4 p-4">
        <p className="text-xs leading-relaxed text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.bPatternNote")}</p>

        <div className="grid gap-3 sm:grid-cols-2">
          <label className="block text-xs font-medium text-ink dark:text-ink-dark">
            {t("settings.gatewayServe.bindLabel")}
            <input
              type="text"
              className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
              value={bind}
              onChange={(e) => setSettings({ gatewayServeBind: e.target.value })}
              spellCheck={false}
              autoComplete="off"
            />
          </label>
          <label className="block text-xs font-medium text-ink dark:text-ink-dark">
            {t("settings.gatewayServe.tokenLabel")}
            <input
              type="password"
              className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
              value={token}
              onChange={(e) => setSettings({ gatewayServeToken: e.target.value })}
              spellCheck={false}
              autoComplete="off"
              placeholder={t("settings.gatewayServe.tokenPlaceholder")}
            />
          </label>
        </div>

        <label className="block text-xs font-medium text-ink dark:text-ink-dark">
          {t("settings.gatewayServe.artifactDirLabel")}
          <input
            type="text"
            className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
            value={artifactDir}
            onChange={(e) => setSettings({ gatewayArtifactDir: e.target.value })}
            spellCheck={false}
            autoComplete="off"
            placeholder={t("settings.gatewayServe.artifactDirPlaceholder")}
          />
          <p className="mt-1 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
            {t("settings.gatewayServe.artifactDirHint")}
          </p>
        </label>

        <div className="grid gap-3 sm:grid-cols-2">
          <div className="rounded-lg border border-dashed border-border px-3 py-2.5 dark:border-border-dark">
            <div className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.urlsHeading")}
            </div>
            <div className="space-y-1.5 pt-2 text-xs">
              <div>
                <span className="text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.healthUrl")}</span>
                <p className="mt-0.5 break-all font-mono text-ink dark:text-ink-dark">{healthUrl}</p>
              </div>
              <div>
                <span className="text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.webhookUrl")}</span>
                <p className="mt-0.5 break-all font-mono text-ink dark:text-ink-dark">{webhookUrl}</p>
              </div>
              {artifactDir.trim() ? (
                <div>
                  <span className="text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.artifactUrl")}</span>
                  <p className="mt-0.5 break-all font-mono text-ink dark:text-ink-dark">{artifactUrl}</p>
                </div>
              ) : null}
            </div>
          </div>
        </div>

        <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
          {t("settings.gatewayServe.healthPrerequisite")}
        </p>

        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void copyText(startCmd, t("settings.environment.clipboardOk"))}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {t("settings.gatewayServe.copyStartCmd")}
          </button>
          <button
            type="button"
            disabled={healthBusy}
            onClick={() => void runHealthCheck()}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {healthBusy ? t("settings.gatewayServe.healthChecking") : t("settings.gatewayServe.healthCheck")}
          </button>
          {healthLabel === "ok" ? (
            <span className="self-center text-xs font-medium text-emerald-700 dark:text-emerald-300">
              {t("settings.gatewayServe.healthBadgeOk")}
            </span>
          ) : null}
          {healthLabel === "fail" ? (
            <span className="self-center text-xs font-medium text-amber-800 dark:text-amber-200">
              {t("settings.gatewayServe.healthBadgeFail")}
            </span>
          ) : null}
        </div>

        <pre className="max-h-32 overflow-auto rounded-md border border-border bg-ink/[0.03] p-2 text-[11px] leading-snug text-ink dark:border-border-dark dark:bg-white/[0.04] dark:text-ink-dark">
          {startCmd}
        </pre>

        <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.webhookAuthHint")}</p>
      </div>
    </section>
  );
}
