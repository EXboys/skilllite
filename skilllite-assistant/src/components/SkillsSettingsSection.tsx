import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { open as openUrl } from "@tauri-apps/plugin-shell";
import { useSettingsStore } from "../stores/useSettingsStore";
import { getLocale, translate, useI18n } from "../i18n";

const SKILL_LIST_MAX_HEIGHT = "min(55vh, 26rem)";

interface DesktopSkillInfo {
  name: string;
  description?: string | null;
  skillType: "script" | "bash_tool" | "prompt_only" | string;
  source?: string | null;
  trustTier: "trusted" | "reviewed" | "community" | "unknown" | string;
  trustScore?: number | null;
  admissionRisk?: "safe" | "suspicious" | "malicious" | string | null;
  dependencyType: "python" | "node" | "none" | string;
  dependencyPackages: string[];
  missingCommands: string[];
  missingAnyCommandGroups: string[][];
  missingEnvVars: string[];
}

type TranslateFn = (key: string, vars?: Record<string, string | number>) => string;

function skillDisplayName(name: string, locale: "zh" | "en"): string {
  if (locale === "en") return name;
  const map: Record<string, string> = {
    "xiaohongshu-writer": "小红书",
    "check-weather-forecast": "天气预报",
    "data-analysis": "数据分析",
    "http-request": "HTTP 请求",
    "nodejs-test": "Node 测试",
    "skill-creator": "技能创建",
    "text-processor": "文本处理",
    weather: "天气",
    calculator: "计算器",
  };
  return map[name] || name;
}

function skillTypeLabel(skill: DesktopSkillInfo, t: TranslateFn): string {
  switch (skill.skillType) {
    case "script":
      return t("status.skillTypeScript");
    case "bash_tool":
      return t("status.skillTypeBashTool");
    default:
      return t("status.skillTypePromptOnly");
  }
}

function trustTierLabel(skill: DesktopSkillInfo, t: TranslateFn): string {
  switch (skill.trustTier) {
    case "trusted":
      return t("status.skillTrustTrusted");
    case "reviewed":
      return t("status.skillTrustReviewed");
    case "community":
      return t("status.skillTrustCommunity");
    default:
      return t("status.skillTrustUnknown");
  }
}

function admissionRiskLabel(
  admissionRisk: DesktopSkillInfo["admissionRisk"],
  t: TranslateFn
): string | null {
  switch (admissionRisk) {
    case "safe":
      return t("status.skillAdmissionSafe");
    case "suspicious":
      return t("status.skillAdmissionSuspicious");
    case "malicious":
      return t("status.skillAdmissionMalicious");
    default:
      return null;
  }
}

function skillMissingHints(skill: DesktopSkillInfo, t: TranslateFn): string[] {
  return [
    ...skill.missingCommands.map((name) => t("status.skillMissingCommand", { name })),
    ...skill.missingAnyCommandGroups.map((names) =>
      t("status.skillMissingAnyCommand", { names: names.join(" / ") })
    ),
    ...skill.missingEnvVars.map((name) => t("status.skillMissingEnv", { name })),
  ];
}

function compactSourceLabel(source?: string | null): string {
  if (!source) return "";
  const trimmed = source.trim();
  if (!trimmed) return "";
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || trimmed;
}

const SKILLS_SH_URL = "https://skills.sh/";

/**
 * Workspace skills: list, add, ZIP import, repair/remove, metadata — lives under Settings → Skills.
 */
export default function SkillsSettingsSection() {
  const { t } = useI18n();
  const locale = getLocale();
  const { settings } = useSettingsStore();
  const workspace = settings.workspace || ".";
  const [skills, setSkills] = useState<DesktopSkillInfo[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loadingList, setLoadingList] = useState(false);
  const [repairing, setRepairing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [repairResult, setRepairResult] = useState<string | null>(null);
  const [resultIsError, setResultIsError] = useState(false);
  const [addSource, setAddSource] = useState("");
  const [adding, setAdding] = useState(false);
  const [addResult, setAddResult] = useState<string | null>(null);
  const [addResultIsError, setAddResultIsError] = useState(false);
  const [initializing, setInitializing] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);

  const loadSkills = useCallback(async (opts?: { preserveRepairResult?: boolean }) => {
    setLoadingList(true);
    if (!opts?.preserveRepairResult) {
      setRepairResult(null);
    }
    try {
      const nextSkills = await invoke<DesktopSkillInfo[]>("skilllite_list_skills", { workspace });
      setSkills(nextSkills);
      if (!opts?.preserveRepairResult) {
        setSelected(new Set());
      } else {
        setSelected((prev) => {
          const names = new Set(nextSkills.map((s) => s.name));
          const next = new Set<string>();
          for (const n of prev) {
            if (names.has(n)) next.add(n);
          }
          return next;
        });
      }
    } catch (e) {
      console.error("[skilllite-assistant] skilllite_list_skills failed:", e);
      setSkills([]);
    } finally {
      setLoadingList(false);
    }
  }, [workspace]);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const toggleOne = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const selectAll = () => setSelected(new Set(skills.map((skill) => skill.name)));
  const selectNone = () => setSelected(new Set());

  const runRepair = async () => {
    setRepairing(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const toRepair = selected.size > 0 ? Array.from(selected) : [];
      const out = await invoke<string>("skilllite_repair_skills", {
        workspace,
        skillNames: toRepair,
      });
      setRepairResult(out || t("status.repairComplete"));
      await loadSkills({ preserveRepairResult: true });
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setRepairing(false);
    }
  };

  /** 始终修复工作区内全部技能（忽略勾选） */
  const runRepairAll = async () => {
    setRepairing(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const out = await invoke<string>("skilllite_repair_skills", {
        workspace,
        skillNames: [],
      });
      setRepairResult(out || t("status.repairComplete"));
      await loadSkills({ preserveRepairResult: true });
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setRepairing(false);
    }
  };

  const runRepairOne = async (name: string) => {
    setRepairing(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const out = await invoke<string>("skilllite_repair_skills", {
        workspace,
        skillNames: [name],
      });
      setRepairResult(out || t("status.repairComplete"));
      await loadSkills({ preserveRepairResult: true });
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setRepairing(false);
    }
  };

  const runDeleteForNames = async (names: string[]) => {
    if (names.length === 0) return;
    setDeleting(true);
    setRepairResult(null);
    setResultIsError(false);
    try {
      const out = await invoke<string>("skilllite_remove_skills", {
        workspace,
        skillNames: names,
      });
      setRepairResult(out || t("status.deleteSkill"));
      setSelected((prev) => {
        const next = new Set(prev);
        for (const n of names) next.delete(n);
        return next;
      });
      await loadSkills({ preserveRepairResult: true });
    } catch (e) {
      setRepairResult(String(e));
      setResultIsError(true);
    } finally {
      setDeleting(false);
    }
  };

  const runDeleteOne = async (name: string) => {
    if (!window.confirm(t("status.deleteSkillConfirm", { n: 1 }))) return;
    await runDeleteForNames([name]);
  };

  const runDelete = async () => {
    if (selected.size === 0) {
      useUiToastStore.getState().show(t("status.deleteSkillNeedSelect"), "error");
      return;
    }
    const names = Array.from(selected);
    if (!window.confirm(t("status.deleteSkillConfirm", { n: names.length }))) return;
    await runDeleteForNames(names);
  };

  const runAdd = async (sourceOverride?: string) => {
    const source = (sourceOverride ?? addSource).trim();
    if (!source) return;
    setAdding(true);
    setAddResult(null);
    setAddResultIsError(false);
    if (sourceOverride) setAddSource("");
    try {
      const out = await invoke<string>("skilllite_add_skill", {
        workspace,
        source,
        force: false,
      });
      setAddResult(out || t("status.skillAdded"));
      setAddSource("");
      loadSkills();
    } catch (e) {
      setAddResult(formatInvokeError(e));
      setAddResultIsError(true);
    } finally {
      setAdding(false);
    }
  };

  const runImportZip = async () => {
    if (adding) return;
    try {
      const selectedPaths = await openFileDialog({
        multiple: false,
        filters: [
          {
            name: "ZIP",
            extensions: ["zip"],
          },
        ],
      });
      if (selectedPaths == null) return;
      const path = Array.isArray(selectedPaths) ? selectedPaths[0] : selectedPaths;
      if (typeof path !== "string" || !path.trim()) return;
      await runAdd(path);
    } catch (e) {
      setAddResult(t("status.zipPickerFailed", { err: formatInvokeError(e) }));
      setAddResultIsError(true);
    }
  };

  const runInitSkills = async () => {
    setInitializing(true);
    setInitError(null);
    try {
      await invoke("skilllite_init_workspace", { dir: workspace });
      await loadSkills();
    } catch (e) {
      setInitError(String(e));
    } finally {
      setInitializing(false);
    }
  };

  return (
    <div className="space-y-4 min-w-0">
      <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1 min-w-0">
        <span className="font-medium text-ink dark:text-ink-dark shrink-0">{t("status.skills")}</span>
        {skills.length > 0 && (
          <span className="text-xs text-ink-mute dark:text-ink-dark-mute shrink-0">
            {t("status.countSkills", { n: skills.length })}
          </span>
        )}
        <div className="flex items-center gap-0.5 min-w-0">
          <button
            type="button"
            onClick={() => void loadSkills()}
            disabled={loadingList}
            className="p-1.5 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
            title={t("status.refresh")}
            aria-label={t("status.refresh")}
          >
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
              className={loadingList ? "animate-spin" : ""}
            >
              <path d="M21 2v6h-6" />
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
              <path d="M3 22v-6h6" />
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            </svg>
          </button>
          <button
            type="button"
            onClick={selectAll}
            disabled={skills.length === 0}
            className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
            title={t("status.selectAll")}
          >
            {t("status.selectAll")}
          </button>
          <button
            type="button"
            onClick={selectNone}
            className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 transition-colors"
            title={t("status.selectNoneTitle")}
          >
            {t("status.deselect")}
          </button>
          {skills.length > 0 && (
            <button
              type="button"
              onClick={() => void runRepairAll()}
              disabled={repairing || deleting}
              className="text-xs px-1.5 py-1 rounded-md text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 transition-colors"
              title={t("status.repairAll")}
            >
              {repairing ? t("status.repairing") : t("status.repairAllToolbar")}
            </button>
          )}
        </div>
      </div>

      <div className="flex flex-col gap-1.5">
        <div className="flex gap-1.5 flex-wrap sm:flex-nowrap">
          <input
            type="text"
            value={addSource}
            onChange={(e) => setAddSource(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && runAdd()}
            placeholder={t("status.skillPlaceholder")}
            className="flex-1 min-w-[12rem] rounded-lg border border-border dark:border-border-dark bg-gray-50 dark:bg-surface-dark px-2.5 py-1.5 text-sm placeholder:text-ink-mute dark:placeholder:text-ink-dark-mute"
          />
          <button
            type="button"
            onClick={runImportZip}
            disabled={adding}
            className="shrink-0 px-2.5 py-1.5 rounded-lg border border-border dark:border-border-dark bg-white dark:bg-surface-dark text-xs font-medium text-ink dark:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {t("status.importZip")}
          </button>
          <button
            type="button"
            onClick={() => runAdd()}
            disabled={adding || !addSource.trim()}
            className="shrink-0 px-2.5 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {adding ? t("status.addingSkill") : t("status.addSkill")}
          </button>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => openUrl(SKILLS_SH_URL)}
            className="text-xs text-ink-mute dark:text-ink-dark-mute hover:text-accent hover:underline text-left"
          >
            {t("status.browseSkillsSh")}
          </button>
          {addResult != null && (
            <span
              className={`min-w-0 max-w-full break-words text-xs ${addResultIsError ? "text-red-600 dark:text-red-400" : "text-ink-mute dark:text-ink-dark-mute"}`}
            >
              {addResult}
            </span>
          )}
        </div>
      </div>

      <div
        className="min-w-0 max-w-full rounded-lg border border-border dark:border-border-dark bg-gray-50/50 dark:bg-surface-dark/50 overflow-x-hidden overflow-y-auto"
        style={{ maxHeight: SKILL_LIST_MAX_HEIGHT }}
      >
        {loadingList ? (
          <div className="p-4 flex items-center justify-center gap-2 text-xs text-ink-mute dark:text-ink-dark-mute">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="animate-spin shrink-0"
            >
              <path d="M21 12a9 9 0 1 1-6.219-8.56" />
            </svg>
            {t("common.loading")}
          </div>
        ) : skills.length === 0 ? (
          <div className="p-4 text-xs text-ink-mute dark:text-ink-dark-mute text-center leading-relaxed">
            <p>{t("status.noSkillsFound")}</p>
            <p className="text-[11px] mt-1 mb-2">{t("status.noSkillsHint")}</p>
            <button
              type="button"
              onClick={runInitSkills}
              disabled={initializing}
              className="px-3 py-1.5 rounded-lg bg-accent text-white text-xs font-medium hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {initializing ? t("status.initializing") : t("status.initSkills")}
            </button>
            {initError && (
              <p className="mt-2 text-xs text-red-600 dark:text-red-400 break-words text-left">{initError}</p>
            )}
          </div>
        ) : (
          <ul className="p-1.5 space-y-0.5">
            {skills.map((skill) => {
              const name = skill.name;
              const riskLabel = admissionRiskLabel(skill.admissionRisk, t);
              const missingHints = skillMissingHints(skill, t);
              return (
                <li key={name}>
                  <label
                    title={name}
                    className={`flex items-start gap-2.5 px-2.5 py-1.5 rounded-md cursor-pointer transition-colors ${
                      selected.has(name)
                        ? "bg-accent/10 dark:bg-accent/20 text-accent dark:text-accent"
                        : "hover:bg-ink/5 dark:hover:bg-white/5 text-ink dark:text-ink-dark"
                    }`}
                  >
                    <input
                      type="checkbox"
                      checked={selected.has(name)}
                      onChange={() => toggleOne(name)}
                      className="rounded border-border dark:border-border-dark text-accent focus:ring-accent/40 shrink-0"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="flex min-w-0 items-center gap-1.5">
                        <span className="truncate text-xs font-medium min-w-0">
                          {skillDisplayName(name, locale)}
                        </span>
                        <span className="shrink-0 rounded-full border border-border dark:border-border-dark bg-white/80 dark:bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-ink-mute dark:text-ink-dark-mute">
                          {skillTypeLabel(skill, t)}
                        </span>
                        {missingHints.length > 0 && (
                          <span className="shrink-0 rounded-full border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/30 px-1.5 py-0.5 text-[10px] font-medium text-amber-700 dark:text-amber-300">
                            {t("status.skillNeedsSetup")}
                          </span>
                        )}
                      </div>
                      <div className="mt-0.5 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5 text-[10px] text-ink-mute dark:text-ink-dark-mute">
                        {skill.source && (
                          <span className="truncate max-w-full">
                            {t("status.skillSourceCompact", { source: compactSourceLabel(skill.source) })}
                          </span>
                        )}
                        <span>
                          {t("status.skillTrustCompact", {
                            tier: trustTierLabel(skill, t),
                            score: skill.trustScore ?? 0,
                          })}
                        </span>
                        {riskLabel && (
                          <span>{t("status.skillAdmissionCompact", { risk: riskLabel })}</span>
                        )}
                      </div>
                    </div>
                    <div className="flex shrink-0 items-start gap-0.5 pt-0.5">
                      <button
                        type="button"
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          invoke("skilllite_open_skill_directory", { workspace, skillName: name }).catch((err) => {
                            console.error("[skilllite-assistant] open_skill_directory failed:", err);
                            const msg = formatInvokeError(err);
                            setRepairResult(translate("status.openSkillDirResult", { err: msg }));
                            setResultIsError(true);
                            useUiToastStore
                              .getState()
                              .show(translate("status.openSkillDirFailed", { err: msg }), "error");
                          });
                        }}
                        className="p-1 rounded text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 shrink-0"
                        title={t("status.openFolder")}
                        aria-label={t("status.openFolderAria")}
                      >
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
                        >
                          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                        </svg>
                      </button>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          void runRepairOne(name);
                        }}
                        disabled={repairing || deleting}
                        className="p-1 rounded text-ink-mute hover:text-ink dark:hover:text-ink-dark hover:bg-ink/5 dark:hover:bg-white/5 shrink-0 disabled:opacity-50"
                        title={t("status.repairThisSkillTitle")}
                        aria-label={t("status.repairThisSkillAria")}
                      >
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
                        >
                          <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
                        </svg>
                      </button>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          void runDeleteOne(name);
                        }}
                        disabled={repairing || deleting}
                        className="p-1 rounded text-ink-mute hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 shrink-0 disabled:opacity-50"
                        title={t("status.deleteThisSkillTitle")}
                        aria-label={t("status.deleteThisSkillAria")}
                      >
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
                        >
                          <polyline points="3 6 5 6 21 6" />
                          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                          <line x1="10" y1="11" x2="10" y2="17" />
                          <line x1="14" y1="11" x2="14" y2="17" />
                        </svg>
                      </button>
                    </div>
                  </label>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* 仅多选：批量修复/删除；单选与未选用行内或顶栏「全部修复」 */}
      {skills.length > 0 && selected.size >= 2 && (
        <div className="flex gap-2 min-w-0">
          <button
            type="button"
            onClick={() => void runRepair()}
            disabled={repairing || deleting}
            className="flex-1 min-w-0 text-sm px-3 py-2 rounded-lg bg-accent text-white font-medium hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {repairing ? t("status.repairing") : t("status.repairSelected", { n: selected.size })}
          </button>
          <button
            type="button"
            onClick={() => void runDelete()}
            disabled={deleting || repairing}
            className="flex-1 min-w-0 text-sm px-3 py-2 rounded-lg border border-red-300 dark:border-red-800/80 text-red-700 dark:text-red-300 font-medium hover:bg-red-50 dark:hover:bg-red-950/30 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {deleting ? t("status.deletingSkills") : t("status.deleteSelected", { n: selected.size })}
          </button>
        </div>
      )}

      {repairResult !== null && (
        <div
          className={`max-w-full min-w-0 break-words p-2.5 rounded-lg text-xs whitespace-pre-wrap max-h-40 overflow-y-auto overflow-x-hidden ${
            resultIsError
              ? "bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300 border border-red-200 dark:border-red-800/50"
              : "bg-green-50 dark:bg-green-900/20 text-green-800 dark:text-green-200 border border-green-200 dark:border-green-800/50"
          }`}
        >
          {repairResult}
        </div>
      )}
    </div>
  );
}
