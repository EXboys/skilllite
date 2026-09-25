import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { RUNTIME_STATUS_REFRESH_EVENT } from "../utils/runtimeStatusRefresh";
import { translate } from "../i18n";

export type RuntimeSource = "system" | "cache" | "none";

export interface RuntimeUiLine {
  source: RuntimeSource;
  label: string;
  revealPath: string | null;
  detail?: string | null;
}

export interface RuntimeUiSnapshot {
  python: RuntimeUiLine;
  node: RuntimeUiLine;
  cacheRoot: string | null;
  cacheRootAbs: string | null;
}

interface ProvisionRuntimeItem {
  requested: boolean;
  ok: boolean;
  message: string;
}

interface ProvisionRuntimesResult {
  python: ProvisionRuntimeItem;
  node: ProvisionRuntimeItem;
}

export function needsFirstTimeRuntimeProvision(r: RuntimeUiSnapshot | null): boolean {
  if (!r) return true;
  return r.python.source === "none" || r.node.source === "none";
}

export type ProvisionPhase = "pending" | "python" | "node";

/** Python/Node 运行时探测与一键下载到 SkillLite 缓存（与会话侧栏原逻辑一致）。 */
export function useRuntimeProvisioning() {
  const [runtime, setRuntime] = useState<RuntimeUiSnapshot | null>(null);
  const [provisionBusy, setProvisionBusy] = useState(false);
  const [provisionNote, setProvisionNote] = useState<string | null>(null);
  const [provisionProgress, setProvisionProgress] = useState<{
    phase: ProvisionPhase;
    message: string;
    percent: number | null;
  } | null>(null);
  const provisioningRef = useRef(false);

  const loadRuntime = useCallback((): Promise<void> => {
    return invoke<RuntimeUiSnapshot>("skilllite_runtime_status")
      .then((r) => setRuntime(r))
      .catch(() => setRuntime(null))
      .then(() => undefined);
  }, []);

  useEffect(() => {
    loadRuntime();
  }, [loadRuntime]);

  useEffect(() => {
    const onVisibility = () => {
      if (document.visibilityState === "visible") loadRuntime();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [loadRuntime]);

  useEffect(() => {
    window.addEventListener(RUNTIME_STATUS_REFRESH_EVENT, loadRuntime);
    return () => window.removeEventListener(RUNTIME_STATUS_REFRESH_EVENT, loadRuntime);
  }, [loadRuntime]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<{
      phase?: string;
      message?: string;
      percent?: number | null;
    }>("skilllite-runtime-provision-progress", (ev) => {
      const msg = ev.payload?.message ?? "";
      if (!msg) return;
      const rawPhase = ev.payload?.phase ?? "";
      const phase: ProvisionPhase =
        rawPhase === "node" ? "node" : rawPhase === "python" ? "python" : "pending";
      let percent: number | null = null;
      const p = ev.payload?.percent;
      if (typeof p === "number" && !Number.isNaN(p)) {
        percent = Math.min(100, Math.max(0, Math.round(p)));
      }
      setProvisionProgress({
        phase,
        message: msg,
        percent,
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const revealInFileManager = useCallback((path: string | null | undefined) => {
    if (!path?.trim()) return;
    invoke("skilllite_reveal_in_file_manager", { path: path.trim() }).catch((err) => {
      console.error("[skilllite-assistant] reveal_in_file_manager failed:", err);
      useUiToastStore
        .getState()
        .show(translate("toast.revealFmFailed", { err: formatInvokeError(err) }), "error");
    });
  }, []);

  const runProvision = useCallback(
    async (force: boolean) => {
      if (provisioningRef.current) return;
      provisioningRef.current = true;
      setProvisionBusy(true);
      setProvisionNote(null);
      const firstTime = needsFirstTimeRuntimeProvision(runtime);
      setProvisionProgress({
        phase: "pending",
        message: force
          ? translate("runtime.provision.force")
          : firstTime
            ? translate("runtime.provision.first")
            : translate("runtime.provision.update"),
        percent: null,
      });
      try {
        const r = await invoke<ProvisionRuntimesResult>("skilllite_provision_runtimes", {
          python: true,
          node: true,
          force,
        });
        const parts: string[] = [];
        if (r.python.requested) {
          parts.push(
            translate(
              r.python.ok ? "runtime.provisionLine.pythonOk" : "runtime.provisionLine.pythonFail",
              { msg: r.python.message }
            )
          );
        }
        if (r.node.requested) {
          parts.push(
            translate(
              r.node.ok ? "runtime.provisionLine.nodeOk" : "runtime.provisionLine.nodeFail",
              { msg: r.node.message }
            )
          );
        }
        setProvisionNote(parts.join(" · ") || translate("runtime.provisionDone"));
        loadRuntime();
      } catch (e) {
        setProvisionNote(e instanceof Error ? e.message : String(e));
      } finally {
        setProvisionProgress(null);
        provisioningRef.current = false;
        setProvisionBusy(false);
      }
    },
    [loadRuntime, runtime]
  );

  return {
    runtime,
    loadRuntime,
    provisionBusy,
    provisionNote,
    provisionProgress,
    runProvision,
    revealInFileManager,
  };
}
