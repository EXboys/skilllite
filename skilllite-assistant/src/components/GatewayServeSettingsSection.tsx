import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { useSettingsStore } from "../stores/useSettingsStore";
import { useUiToastStore } from "../stores/useUiToastStore";
import { formatInvokeError } from "../utils/formatInvokeError";

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

function trimToNull(raw: string | undefined): string | null {
  const t = raw?.trim() ?? "";
  return t ? t : null;
}

function pushExportLine(lines: string[], name: string, value: string | undefined): void {
  const v = value?.trim();
  if (v) {
    lines.push(`export ${name}=${shellQuoteSingle(v)}`);
  }
}

function buildStartCommand(opts: {
  bind: string;
  token?: string;
  artifactDir?: string;
  dingtalkWebhook?: string;
  dingtalkSecret?: string;
  feishuWebhook?: string;
  feishuSecret?: string;
  telegramBotToken?: string;
  telegramChatId?: string;
}): string {
  const lines: string[] = [];
  pushExportLine(lines, "SKILLLITE_CHANNEL_DINGTALK_WEBHOOK", opts.dingtalkWebhook);
  pushExportLine(lines, "SKILLLITE_CHANNEL_DINGTALK_SECRET", opts.dingtalkSecret);
  pushExportLine(lines, "SKILLLITE_CHANNEL_FEISHU_WEBHOOK", opts.feishuWebhook);
  pushExportLine(lines, "SKILLLITE_CHANNEL_FEISHU_SECRET", opts.feishuSecret);
  pushExportLine(lines, "SKILLLITE_CHANNEL_TELEGRAM_BOT_TOKEN", opts.telegramBotToken);
  pushExportLine(lines, "SKILLLITE_CHANNEL_TELEGRAM_CHAT_ID", opts.telegramChatId);

  const b = opts.bind.trim() || DEFAULT_BIND;
  let main = `SKILLLITE_GATEWAY_SERVE_ALLOW=1 skilllite gateway serve --bind ${b}`;
  const t = opts.token?.trim();
  if (t) {
    main += ` --token ${shellQuoteSingle(t)}`;
  }
  const a = opts.artifactDir?.trim();
  if (a) {
    main += ` --artifact-dir ${shellQuoteSingle(a)}`;
  }
  if (lines.length > 0) {
    return `${lines.join("\n")}\n${main}`;
  }
  return main;
}

interface GatewayManagedStatus {
  running: boolean;
  source: "none" | "managed" | "external";
  pid?: number;
  bind?: string;
  startedAtMs?: number;
  lastExitCode?: number;
  lastError?: string;
  recentLog?: string;
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
  const [gatewayBusy, setGatewayBusy] = useState<"start" | "stop" | null>(null);
  const [gatewayStatus, setGatewayStatus] = useState<GatewayManagedStatus | null>(null);

  const bind = settings.gatewayServeBind ?? DEFAULT_BIND;
  const token = settings.gatewayServeToken ?? "";
  const artifactDir = settings.gatewayArtifactDir ?? "";
  const dingtalkWebhook = settings.gatewayChannelDingtalkWebhook ?? "";
  const dingtalkSecret = settings.gatewayChannelDingtalkSecret ?? "";
  const feishuWebhook = settings.gatewayChannelFeishuWebhook ?? "";
  const feishuSecret = settings.gatewayChannelFeishuSecret ?? "";
  const telegramBotToken = settings.gatewayChannelTelegramBotToken ?? "";
  const telegramChatId = settings.gatewayChannelTelegramChatId ?? "";
  const workspace = settings.workspace;

  const baseUrl = useMemo(() => httpBaseForLocalFetch(bind), [bind]);
  const healthUrl = `${baseUrl}/health`;
  const webhookUrl = `${baseUrl}/webhook/inbound`;
  const artifactUrl = `${baseUrl}/v1/runs/<run_id>/artifacts?key=<key>`;
  const startCmd = useMemo(
    () =>
      buildStartCommand({
        bind,
        token: token || undefined,
        artifactDir: artifactDir || undefined,
        dingtalkWebhook: dingtalkWebhook || undefined,
        dingtalkSecret: dingtalkSecret || undefined,
        feishuWebhook: feishuWebhook || undefined,
        feishuSecret: feishuSecret || undefined,
        telegramBotToken: telegramBotToken || undefined,
        telegramChatId: telegramChatId || undefined,
      }),
    [
      artifactDir,
      bind,
      dingtalkSecret,
      dingtalkWebhook,
      feishuSecret,
      feishuWebhook,
      telegramBotToken,
      telegramChatId,
      token,
    ]
  );
  const gatewaySource = gatewayStatus?.source ?? "none";
  const gatewayRunning = gatewayStatus?.running ?? false;
  const managedRunning = gatewayRunning && gatewaySource === "managed";
  const externalRunning = gatewayRunning && gatewaySource === "external";
  const gatewayBind = gatewayStatus?.bind?.trim() ?? "";
  const bindChangedWhileRunning = managedRunning && gatewayBind.length > 0 && gatewayBind !== bind.trim();

  const copyText = useCallback(async (text: string, okMsg: string) => {
    try {
      await navigator.clipboard.writeText(text);
      useUiToastStore.getState().show(okMsg, "info");
    } catch {
      useUiToastStore.getState().show(t("settings.gatewayServe.clipboardFail"), "error");
    }
  }, [t]);

  const refreshGatewayStatus = useCallback(async () => {
    try {
      const status = await invoke<GatewayManagedStatus>("assistant_gateway_status", {
        request: { bind },
      });
      setGatewayStatus(status);
      return status;
    } catch (e) {
      setGatewayStatus((prev) => ({
        running: false,
        source: "none",
        pid: prev?.pid,
        bind,
        startedAtMs: prev?.startedAtMs,
        lastExitCode: prev?.lastExitCode,
        lastError: formatInvokeError(e),
        recentLog: prev?.recentLog,
      }));
      return null;
    }
  }, [bind]);

  useEffect(() => {
    void refreshGatewayStatus();
    const timer = window.setInterval(() => {
      void refreshGatewayStatus();
    }, 3000);
    return () => window.clearInterval(timer);
  }, [refreshGatewayStatus]);

  const startManagedGateway = useCallback(async () => {
    setGatewayBusy("start");
    try {
      const status = await invoke<GatewayManagedStatus>("assistant_gateway_start", {
        request: {
          workspace,
          bind,
          token: token || null,
          artifactDir: artifactDir || null,
          gatewayChannelDingtalkWebhook: trimToNull(dingtalkWebhook),
          gatewayChannelDingtalkSecret: trimToNull(dingtalkSecret),
          gatewayChannelFeishuWebhook: trimToNull(feishuWebhook),
          gatewayChannelFeishuSecret: trimToNull(feishuSecret),
          gatewayChannelTelegramBotToken: trimToNull(telegramBotToken),
          gatewayChannelTelegramChatId: trimToNull(telegramChatId),
        },
      });
      setGatewayStatus(status);
      setHealthLabel("idle");
      useUiToastStore.getState().show(
        status.source === "external"
          ? t("settings.gatewayServe.externalDetected")
          : t("settings.gatewayServe.startedOk"),
        "info"
      );
    } catch (e) {
      const err = formatInvokeError(e);
      useUiToastStore.getState().show(t("settings.gatewayServe.startFailed", { err }), "error");
      await refreshGatewayStatus();
    } finally {
      setGatewayBusy(null);
    }
  }, [
    artifactDir,
    bind,
    dingtalkSecret,
    dingtalkWebhook,
    feishuSecret,
    feishuWebhook,
    refreshGatewayStatus,
    t,
    telegramBotToken,
    telegramChatId,
    token,
    workspace,
  ]);

  const stopManagedGateway = useCallback(async () => {
    setGatewayBusy("stop");
    try {
      const status = await invoke<GatewayManagedStatus>("assistant_gateway_stop");
      setGatewayStatus(status);
      setHealthLabel("idle");
      useUiToastStore.getState().show(t("settings.gatewayServe.stoppedOk"), "info");
    } catch (e) {
      const err = formatInvokeError(e);
      useUiToastStore.getState().show(t("settings.gatewayServe.stopFailed", { err }), "error");
      await refreshGatewayStatus();
    } finally {
      setGatewayBusy(null);
    }
  }, [refreshGatewayStatus, t]);

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

      <div className="space-y-5 p-4">
        {/* 入站：本机 HTTP 监听与操作 */}
        <div
          className="rounded-lg border border-border bg-white dark:border-border-dark dark:bg-paper-dark"
          role="region"
          aria-labelledby="gateway-inbound-section-title"
        >
          <div className="border-b border-border/60 px-3 py-2 sm:px-4 dark:border-border-dark/60">
            <h4
              id="gateway-inbound-section-title"
              className="text-sm font-semibold text-ink dark:text-ink-dark"
            >
              <span className="mr-2 font-mono text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                {t("settings.gatewayServe.sectionInboundBadge")}
              </span>
              {t("settings.gatewayServe.sectionInboundTitle")}
            </h4>
            <p className="mt-0.5 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.sectionInboundIntro")}
            </p>
          </div>
          <div className="space-y-4 p-3 sm:p-4">
        <p className="text-xs leading-relaxed text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.bPatternNote")}</p>

        <div className="rounded-lg border border-dashed border-border px-3 py-2.5 dark:border-border-dark">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.managedStatusHeading")}
            </span>
            <span className={`text-xs font-medium ${gatewayRunning ? "text-ink dark:text-ink-dark" : "text-ink-mute dark:text-ink-dark-mute"}`}>
              {managedRunning
                ? t("settings.gatewayServe.managedStatusRunning")
                : externalRunning
                  ? t("settings.gatewayServe.externalStatusRunning")
                  : t("settings.gatewayServe.managedStatusStopped")}
            </span>
            {managedRunning && gatewayStatus?.pid ? (
              <span className="text-xs text-ink-mute dark:text-ink-dark-mute">
                {t("settings.gatewayServe.managedPid", { pid: String(gatewayStatus.pid) })}
              </span>
            ) : null}
          </div>
          <p className="mt-2 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
            {externalRunning
              ? t("settings.gatewayServe.externalStatusNote")
              : t("settings.gatewayServe.managedStatusNote")}
          </p>
          {gatewayStatus?.bind ? (
            <p className="mt-2 break-all font-mono text-[11px] leading-snug text-ink dark:text-ink-dark">
              {gatewayStatus.bind}
            </p>
          ) : null}
          {!externalRunning && gatewayStatus?.lastError ? (
            <p className="mt-2 text-[11px] leading-snug text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.managedLastError", { msg: gatewayStatus.lastError })}
            </p>
          ) : null}
          {!externalRunning && !gatewayStatus?.lastError && gatewayStatus?.recentLog ? (
            <p className="mt-2 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.managedRecentLog", { msg: gatewayStatus.recentLog })}
            </p>
          ) : null}
          {bindChangedWhileRunning ? (
            <p className="mt-2 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.restartAfterChange")}
            </p>
          ) : null}
        </div>

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
            disabled={gatewayBusy !== null || gatewayRunning}
            onClick={() => void startManagedGateway()}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {gatewayBusy === "start"
              ? t("settings.gatewayServe.managedStarting")
              : externalRunning
                ? t("settings.gatewayServe.externalAlreadyRunning")
              : t("settings.gatewayServe.startManaged")}
          </button>
          <button
            type="button"
            disabled={gatewayBusy !== null || !managedRunning}
            onClick={() => void stopManagedGateway()}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {gatewayBusy === "stop"
              ? t("settings.gatewayServe.managedStopping")
              : externalRunning
                ? t("settings.gatewayServe.externalStopManual")
              : t("settings.gatewayServe.stopManaged")}
          </button>
          <button
            type="button"
            onClick={() => void copyText(startCmd, t("settings.environment.clipboardOk"))}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {t("settings.gatewayServe.copyStartCmd")}
          </button>
          <button
            type="button"
            disabled={healthBusy || gatewayBusy !== null}
            onClick={() => void runHealthCheck()}
            className="inline-flex items-center justify-center rounded-md border border-border bg-white px-3 py-1.5 text-xs font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          >
            {healthBusy ? t("settings.gatewayServe.healthChecking") : t("settings.gatewayServe.healthCheck")}
          </button>
          {healthLabel === "ok" ? (
            <span className="self-center text-xs font-medium text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.healthBadgeOk")}
            </span>
          ) : null}
          {healthLabel === "fail" ? (
            <span className="self-center text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.healthBadgeFail")}
            </span>
          ) : null}
        </div>

        <pre className="max-h-32 overflow-auto rounded-md border border-border bg-ink/[0.03] p-2 text-[11px] leading-snug text-ink dark:border-border-dark dark:bg-white/[0.04] dark:text-ink-dark">
          {startCmd}
        </pre>

        <p className="text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">{t("settings.gatewayServe.webhookAuthHint")}</p>
          </div>
        </div>

        {/* 出站：入站成功后推送到 IM */}
        <div
          className="rounded-lg border border-border bg-white dark:border-border-dark dark:bg-paper-dark"
          role="region"
          aria-labelledby="gateway-outbound-section-title"
        >
          <div className="border-b border-border/60 px-3 py-2 sm:px-4 dark:border-border-dark/60">
            <h4
              id="gateway-outbound-section-title"
              className="text-sm font-semibold text-ink dark:text-ink-dark"
            >
              <span className="mr-2 font-mono text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                {t("settings.gatewayServe.sectionOutboundBadge")}
              </span>
              {t("settings.gatewayServe.sectionOutboundTitle")}
            </h4>
            <p className="mt-0.5 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.gatewayServe.sectionOutboundIntro")}
            </p>
          </div>
          <div className="p-3 sm:p-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.dingtalkWebhookLabel")}
              <input
                type="text"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={dingtalkWebhook}
                onChange={(e) => setSettings({ gatewayChannelDingtalkWebhook: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.dingtalkSecretLabel")}
              <input
                type="password"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={dingtalkSecret}
                onChange={(e) => setSettings({ gatewayChannelDingtalkSecret: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.feishuWebhookLabel")}
              <input
                type="text"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={feishuWebhook}
                onChange={(e) => setSettings({ gatewayChannelFeishuWebhook: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.feishuSecretLabel")}
              <input
                type="password"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={feishuSecret}
                onChange={(e) => setSettings({ gatewayChannelFeishuSecret: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.telegramBotTokenLabel")}
              <input
                type="password"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={telegramBotToken}
                onChange={(e) => setSettings({ gatewayChannelTelegramBotToken: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
            <label className="block text-xs font-medium text-ink dark:text-ink-dark">
              {t("settings.gatewayServe.telegramChatIdLabel")}
              <input
                type="text"
                className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1.5 font-mono text-sm text-ink shadow-sm dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark"
                value={telegramChatId}
                onChange={(e) => setSettings({ gatewayChannelTelegramChatId: e.target.value })}
                spellCheck={false}
                autoComplete="off"
              />
            </label>
          </div>
          </div>
        </div>
      </div>
    </section>
  );
}
