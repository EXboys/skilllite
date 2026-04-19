import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { translate } from "../i18n";
import { useUiToastStore } from "../stores/useUiToastStore";
import { formatRuntimeProvisionProgress } from "../utils/runtimeProvisionProgressZh";
import {
  needsFirstTimeRuntimeProvision,
  useRuntimeProvisioning,
  type RuntimeUiLine,
  type RuntimeSource,
} from "../hooks/useRuntimeProvisioning";

type GitPlat = "windows" | "macos" | "linux";

const GIT_CMD: Record<GitPlat, string> = {
  windows: "winget install --id Git.Git -e --source winget",
  macos: "brew install git",
  linux: "sudo apt install git",
};

function guessGitPlatform(): GitPlat {
  if (typeof navigator === "undefined") return "linux";
  const p = (navigator.platform || "").toLowerCase();
  const ua = (navigator.userAgent || "").toLowerCase();
  if (p.includes("win") || ua.includes("windows")) return "windows";
  if (p.includes("mac") || ua.includes("mac os") || ua.includes("macintosh")) {
    return "macos";
  }
  return "linux";
}

interface GitUiStatusPayload {
  available: boolean;
  versionLine?: string | null;
  errorDetail?: string | null;
}

function runtimeSourceBadgeLabel(source: RuntimeSource): string {
  return translate(`runtime.badge.${source}`);
}

type ProvisionPhase = "pending" | "python" | "node";

function provisionPhaseI18n(phase: ProvisionPhase): string {
  return translate(`runtime.phase.${phase}`);
}

export default function EnvironmentSettingsSection() {
  const { t } = useI18n();
  const {
    runtime,
    provisionBusy,
    provisionNote,
    provisionProgress,
    runProvision,
    revealInFileManager,
    loadRuntime,
  } = useRuntimeProvisioning();

  const [git, setGit] = useState<GitUiStatusPayload | null>(null);
  const [gitLoading, setGitLoading] = useState(false);
  const [platTab, setPlatTab] = useState<GitPlat>(() => guessGitPlatform());

  const activeGitCmd = GIT_CMD[platTab];

  const loadGit = useCallback(async () => {
    setGitLoading(true);
    try {
      const s = await invoke<GitUiStatusPayload>("skilllite_git_status");
      setGit({
        available: s.available,
        versionLine: s.versionLine ?? null,
        errorDetail: s.errorDetail ?? null,
      });
    } catch (e: unknown) {
      setGit({
        available: false,
        versionLine: null,
        errorDetail: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setGitLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadGit();
  }, [loadGit]);

  const refreshBusy = gitLoading || provisionBusy;

  const refreshAll = useCallback(async () => {
    if (refreshBusy) return;
    await Promise.all([loadGit(), loadRuntime()]);
    useUiToastStore.getState().show(t("settings.environment.refreshedOk"), "info");
  }, [loadGit, loadRuntime, refreshBusy, t]);

  const copyText = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      useUiToastStore.getState().show(t("settings.environment.clipboardOk"), "info");
    } catch {
      /* ignore */
    }
  };

  const platTabs = useMemo(
    () =>
      (
        [
          ["windows", "settings.environment.platformWin"],
          ["macos", "settings.environment.platformMac"],
          ["linux", "settings.environment.platformLinux"],
        ] as const
      ).map(([id, labelKey]) => ({
        id: id as GitPlat,
        labelKey,
      })),
    []
  );

  return (
    <div className="space-y-6 max-w-3xl">
      {/* 顶栏说明 + 一键刷新 */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
        <p className="text-sm leading-relaxed text-ink dark:text-ink-dark">{t("settings.environment.intro")}</p>
        <button
          type="button"
          onClick={() => void refreshAll()}
          disabled={refreshBusy}
          className="inline-flex shrink-0 items-center justify-center gap-2 self-start rounded-lg border border-border bg-white px-4 py-2 text-sm font-medium text-ink shadow-sm transition-colors hover:bg-ink/[0.04] disabled:cursor-not-allowed disabled:opacity-50 dark:border-border-dark dark:bg-paper-dark dark:text-ink-dark dark:hover:bg-white/[0.06]"
          aria-busy={refreshBusy}
        >
          <IconRefresh className={`h-4 w-4 ${refreshBusy ? "motion-safe:animate-spin" : ""}`} aria-hidden />
          {t("settings.environment.refreshAll")}
        </button>
      </div>

      {/* Git */}
      <section
        className="overflow-hidden rounded-xl border border-border bg-white dark:border-border-dark dark:bg-paper-dark"
        aria-labelledby="env-git-heading"
      >
        <div className="flex items-start justify-between gap-3 border-b border-border/80 bg-ink/[0.03] px-4 py-3 dark:border-border-dark/80 dark:bg-white/[0.04]">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3 id="env-git-heading" className="text-sm font-semibold text-ink dark:text-ink-dark">
                {t("settings.environment.gitTitle")}
              </h3>
              {!gitLoading && git ? (
                <span
                  className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold ${
                    git.available
                      ? "bg-emerald-500/15 text-emerald-800 dark:text-emerald-300"
                      : "bg-amber-500/15 text-amber-900 dark:text-amber-100"
                  }`}
                >
                  {git.available
                    ? t("settings.environment.gitStatusReady")
                    : t("settings.environment.gitStatusMissing")}
                </span>
              ) : null}
            </div>
            <p className="mt-1 text-xs leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.environment.gitDesc")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void loadGit()}
            disabled={gitLoading}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-ink transition-colors hover:bg-ink/5 disabled:opacity-50 dark:border-border-dark dark:text-ink-dark dark:hover:bg-white/5"
            title={t("settings.environment.gitRecheck")}
          >
            <IconRefresh className={`h-3.5 w-3.5 ${gitLoading ? "motion-safe:animate-spin" : ""}`} aria-hidden />
            {gitLoading ? t("settings.environment.gitChecking") : t("settings.environment.gitRecheck")}
          </button>
        </div>

        <div className="space-y-4 p-4">
          {gitLoading && !git ? (
            <div className="space-y-2" aria-busy="true">
              <div className="h-4 w-40 animate-pulse rounded-md bg-ink/10 dark:bg-white/10" />
              <div className="h-3 w-full max-w-md animate-pulse rounded-md bg-ink/10 dark:bg-white/10" />
            </div>
          ) : null}

          {!gitLoading && git?.available ? (
            <div className="flex items-start gap-3 rounded-lg border border-emerald-500/25 bg-emerald-500/[0.06] px-3 py-2.5 dark:border-emerald-400/30 dark:bg-emerald-500/10">
              <IconCheck className="mt-0.5 h-5 w-5 shrink-0 text-emerald-600 dark:text-emerald-400" aria-hidden />
              <p className="min-w-0 break-all font-mono text-sm text-emerald-900 dark:text-emerald-200">
                {git.versionLine ?? "git"}
              </p>
            </div>
          ) : null}

          {!gitLoading && git && !git.available ? (
            <div className="space-y-4">
              <div className="flex items-start gap-3 rounded-lg border border-amber-500/30 bg-amber-500/[0.07] px-3 py-2.5 dark:border-amber-400/35 dark:bg-amber-500/10">
                <IconAlert className="mt-0.5 h-5 w-5 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />
                <div className="min-w-0 flex-1 space-y-1">
                  <p className="text-sm font-medium text-amber-950 dark:text-amber-100">
                    {t("settings.environment.gitMissing")}
                  </p>
                  {git.errorDetail ? (
                    <details className="text-[11px]">
                      <summary className="cursor-pointer font-medium text-amber-900/90 dark:text-amber-200/90">
                        {t("settings.environment.detectFailed")}
                      </summary>
                      <p className="mt-1 break-all font-mono text-ink-mute dark:text-ink-dark-mute">
                        {git.errorDetail}
                      </p>
                    </details>
                  ) : null}
                </div>
              </div>

              <div>
                <p className="text-xs leading-snug text-ink dark:text-ink-dark">
                  {t("settings.environment.gitInstallHint")}
                </p>
                <p className="mt-3 text-[11px] font-medium uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("settings.environment.installTabsLabel")}
                </p>
                <div className="mt-2 flex flex-wrap gap-2">
                  {platTabs.map(({ id, labelKey }) => (
                    <button
                      key={id}
                      type="button"
                      onClick={() => setPlatTab(id)}
                      className={`rounded-full px-3 py-1.5 text-xs font-medium transition-colors ${
                        platTab === id
                          ? "bg-accent/15 text-accent ring-1 ring-accent/40"
                          : "border border-border bg-ink/[0.03] text-ink-mute hover:bg-ink/5 dark:border-border-dark dark:text-ink-dark-mute dark:hover:bg-white/5"
                      }`}
                    >
                      {t(labelKey)}
                    </button>
                  ))}
                </div>

                <div className="mt-3 flex flex-col gap-2 rounded-lg border border-border bg-ink/[0.03] p-3 dark:border-border-dark dark:bg-white/[0.04]">
                  <code className="block break-all font-mono text-[12px] text-ink dark:text-ink-dark">
                    {activeGitCmd}
                  </code>
                  <div className="flex flex-wrap items-center gap-3">
                    <button
                      type="button"
                      onClick={() => void copyText(activeGitCmd)}
                      className="inline-flex items-center rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-white hover:bg-accent-hover"
                    >
                      {t("settings.environment.copyCmd")}
                    </button>
                    <a
                      href="https://git-scm.com/download"
                      target="_blank"
                      rel="noreferrer"
                      className="text-xs font-medium text-accent hover:underline"
                    >
                      {t("settings.environment.gitDownloadPage")}
                    </a>
                  </div>
                  {platTab === "linux" ? (
                    <p className="text-[11px] text-ink-mute dark:text-ink-dark-mute">
                      {t("settings.environment.cmdLinuxNote")}
                    </p>
                  ) : null}
                </div>
              </div>
            </div>
          ) : null}
        </div>
      </section>

      {/* Python & Node */}
      <section
        className="overflow-hidden rounded-xl border border-border bg-white dark:border-border-dark dark:bg-paper-dark"
        aria-labelledby="env-runtime-heading"
      >
        <div className="flex items-start justify-between gap-3 border-b border-border/80 bg-ink/[0.03] px-4 py-3 dark:border-border-dark/80 dark:bg-white/[0.04]">
          <div className="min-w-0">
            <h3 id="env-runtime-heading" className="text-sm font-semibold text-ink dark:text-ink-dark">
              {t("settings.environment.runtimeTitle")}
            </h3>
            <p className="mt-1 text-xs leading-snug text-ink-mute dark:text-ink-dark-mute">
              {t("settings.environment.runtimeDesc")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void loadRuntime()}
            disabled={provisionBusy}
            className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-ink transition-colors hover:bg-ink/5 disabled:opacity-50 dark:border-border-dark dark:text-ink-dark dark:hover:bg-white/5"
            title={t("settings.environment.refreshStatus")}
          >
            <IconRefresh className="h-3.5 w-3.5" aria-hidden />
            {t("settings.environment.refreshStatus")}
          </button>
        </div>

        <div className="space-y-4 p-4">
          {!runtime ? (
            <div className="grid gap-3 sm:grid-cols-2" aria-busy="true">
              <div className="h-24 animate-pulse rounded-lg bg-ink/10 dark:bg-white/10" />
              <div className="h-24 animate-pulse rounded-lg bg-ink/10 dark:bg-white/10" />
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              {(
                [
                  { key: "py", title: t("session.runtimeRowPython"), line: runtime.python, dot: "bg-sky-500" },
                  { key: "node", title: t("session.runtimeRowNode"), line: runtime.node, dot: "bg-emerald-500" },
                ] as const
              ).map(({ key, title, line, dot }) => (
                <RuntimeCard
                  key={key}
                  title={title}
                  line={line}
                  dotClass={dot}
                  onReveal={() => revealInFileManager(line.revealPath)}
                />
              ))}
            </div>
          )}

          <div className="border-t border-border/70 pt-4 dark:border-border-dark/70">
            {provisionBusy ? (
              <div
                className="rounded-lg border border-accent/30 bg-accent/[0.06] px-3 py-3 dark:bg-accent/10"
                aria-live="polite"
                role="status"
                aria-busy="true"
              >
                <div className="flex items-center justify-between gap-2 min-w-0">
                  <span className="text-xs font-semibold text-accent">{t("session.downloadInProgress")}</span>
                  <div className="flex items-center gap-1.5 min-w-0 justify-end">
                    {provisionProgress ? (
                      <>
                        <span
                          className={`shrink-0 rounded px-1 py-px text-[9px] font-semibold uppercase tracking-wide ${
                            provisionProgress.phase === "python"
                              ? "bg-emerald-500/15 text-emerald-800 dark:text-emerald-300"
                              : provisionProgress.phase === "node"
                                ? "bg-sky-500/15 text-sky-900 dark:text-sky-200"
                                : "bg-ink/10 text-ink-mute dark:text-ink-dark-mute"
                          }`}
                        >
                          {provisionPhaseI18n(provisionProgress.phase)}
                        </span>
                        {provisionProgress.percent != null ? (
                          <span className="text-[9px] font-mono text-ink-mute dark:text-ink-dark-mute tabular-nums shrink-0">
                            {provisionProgress.percent}%
                          </span>
                        ) : null}
                      </>
                    ) : null}
                  </div>
                </div>
                <div
                  className="mt-2 h-1.5 w-full rounded-full bg-ink/10 dark:bg-white/10 overflow-hidden"
                  role="progressbar"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-busy={provisionProgress == null || provisionProgress.percent == null}
                  aria-valuenow={
                    provisionProgress?.percent != null ? provisionProgress.percent : undefined
                  }
                  aria-label={t("session.provisionProgressAria")}
                >
                  {provisionProgress?.percent != null ? (
                    <div
                      className="h-full rounded-full bg-accent transition-[width] duration-200 ease-out"
                      style={{ width: `${provisionProgress.percent}%` }}
                    />
                  ) : (
                    <div className="h-full w-full rounded-full bg-accent/25 relative overflow-hidden">
                      <div className="absolute inset-y-0 left-0 w-2/5 rounded-full bg-accent/90 motion-safe:animate-pulse" />
                    </div>
                  )}
                </div>
                <p className="mt-2 text-[11px] leading-snug text-ink dark:text-ink-dark font-mono break-words">
                  {provisionProgress
                    ? formatRuntimeProvisionProgress(provisionProgress.message)
                    : t("session.provisionWait")}
                </p>
                <p className="mt-1 text-[9px] text-ink-mute dark:text-ink-dark-mute leading-snug">
                  {t("session.provisionKeepOpen")}
                </p>
              </div>
            ) : (
              <div className="flex flex-col gap-2 sm:flex-row">
                <button
                  type="button"
                  onClick={() => void runProvision(false)}
                  title={
                    needsFirstTimeRuntimeProvision(runtime)
                      ? t("session.provisionTitleFirst")
                      : t("session.provisionTitleUpdate")
                  }
                  className="inline-flex flex-1 items-center justify-center gap-2 rounded-lg border border-accent/25 bg-accent/10 px-4 py-2.5 text-sm font-semibold text-accent transition-colors hover:bg-accent/15 dark:border-accent/35"
                >
                  <IconDownload className="h-4 w-4 shrink-0" aria-hidden />
                  {needsFirstTimeRuntimeProvision(runtime)
                    ? t("session.provisionBtnFirst")
                    : t("session.provisionBtnUpdate")}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (!window.confirm(t("session.provisionForceConfirm"))) {
                      return;
                    }
                    void runProvision(true);
                  }}
                  className="inline-flex flex-1 items-center justify-center rounded-lg border border-amber-500/45 bg-amber-500/10 px-4 py-2.5 text-sm font-medium text-amber-950 transition-colors hover:bg-amber-500/15 dark:border-amber-400/40 dark:text-amber-100"
                  title={t("session.provisionForceTitle")}
                >
                  {t("session.provisionForceBtn")}
                </button>
              </div>
            )}
            {provisionNote && !provisionBusy ? (
              <p className="mt-3 rounded-lg border border-border bg-ink/[0.02] px-3 py-2 text-xs leading-snug text-ink-mute dark:border-border-dark dark:bg-white/[0.03] dark:text-ink-dark-mute">
                {provisionNote}
              </p>
            ) : null}
          </div>

          {runtime?.cacheRoot ? (
            <div className="flex flex-col gap-2 rounded-lg border border-dashed border-border px-3 py-2.5 dark:border-border-dark sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <div className="text-[10px] font-semibold uppercase tracking-wide text-ink-mute dark:text-ink-dark-mute">
                  {t("settings.environment.cacheSection")}
                </div>
                <p className="mt-0.5 truncate font-mono text-[11px] text-ink dark:text-ink-dark" title={runtime.cacheRootAbs ?? undefined}>
                  {runtime.cacheRoot}
                </p>
              </div>
              <button
                type="button"
                disabled={!runtime.cacheRootAbs}
                onClick={() => revealInFileManager(runtime.cacheRootAbs)}
                className="inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs font-medium text-ink transition-colors hover:bg-ink/5 disabled:cursor-not-allowed disabled:opacity-50 dark:border-border-dark dark:text-ink-dark dark:hover:bg-white/5"
                title={runtime.cacheRootAbs ? t("session.cacheRevealTitle") : t("session.cacheRevealDisabled")}
              >
                <IconFolderOpen className="h-3.5 w-3.5" aria-hidden />
                {t("status.openFolder")}
              </button>
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function RuntimeCard({
  title,
  line,
  dotClass,
  onReveal,
}: {
  title: string;
  line: RuntimeUiLine;
  dotClass: string;
  onReveal: () => void;
}) {
  const canReveal = Boolean(line.revealPath);
  const badge = (
    <span
      className={`shrink-0 rounded px-1.5 py-px text-[10px] font-medium ${
        line.source === "system"
          ? "bg-emerald-500/15 text-emerald-800 dark:text-emerald-300"
          : line.source === "cache"
            ? "bg-amber-500/15 text-amber-900 dark:text-amber-200"
            : "bg-ink/10 text-ink-mute dark:text-ink-dark-mute"
      }`}
    >
      {runtimeSourceBadgeLabel(line.source)}
    </span>
  );

  const inner = (
    <>
      <div className="flex items-center gap-2">
        <span className={`h-2 w-2 shrink-0 rounded-full ${dotClass}`} aria-hidden />
        <span className="text-xs font-medium text-ink-mute dark:text-ink-dark-mute">{title}</span>
      </div>
      <p className="mt-1 line-clamp-2 break-all text-sm font-medium text-ink dark:text-ink-dark">{line.label}</p>
      {line.detail ? (
        <p className="mt-1 text-[11px] leading-snug text-ink-mute dark:text-ink-dark-mute">{line.detail}</p>
      ) : null}
      <div className="mt-2 flex flex-wrap items-center gap-2">{badge}</div>
    </>
  );

  if (canReveal) {
    return (
      <button
        type="button"
        onClick={onReveal}
        title={translate("session.revealInFmTitle", { label: line.label })}
        className="flex w-full flex-col rounded-lg border border-border bg-ink/[0.02] p-3 text-left transition-colors hover:border-accent/35 hover:bg-accent/[0.04] focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/40 dark:border-border-dark dark:bg-white/[0.03] dark:hover:bg-accent/10"
      >
        {inner}
      </button>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-ink/[0.02] p-3 dark:border-border-dark dark:bg-white/[0.03]">
      {inner}
    </div>
  );
}

function IconRefresh({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M3 3v5h5" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" strokeLinecap="round" strokeLinejoin="round" />
      <path d="M16 16h5v5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function IconCheck({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M20 6 9 17l-5-5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function IconAlert({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M12 9v4" strokeLinecap="round" />
      <path d="M12 17h.01" strokeLinecap="round" />
      <path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function IconDownload({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" strokeLinecap="round" strokeLinejoin="round" />
      <polyline points="7 10 12 15 17 10" strokeLinecap="round" strokeLinejoin="round" />
      <line x1="12" y1="15" x2="12" y2="3" strokeLinecap="round" />
    </svg>
  );
}

function IconFolderOpen({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
