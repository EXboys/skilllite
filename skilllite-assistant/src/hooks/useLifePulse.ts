import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "../stores/useUiToastStore";
import { useSessionStore } from "../stores/useSessionStore";
import { translate } from "../i18n";

export interface LifePulseStatus {
  enabled: boolean;
  alive: boolean;
  growth_running: boolean;
  rhythm_running: boolean;
  workspace: string;
}

interface PulseEvent {
  type: string;
  ts: number;
  detail?: string;
}

export interface LifePulseActivity {
  type: string;
  ts: number;
  detail?: string;
  label: string;
}

const PULSE_I18N_KEYS: Record<string, string> = {
  "growth-started": "lifePulse.growthStarted",
  "growth-done": "lifePulse.growthDone",
  "growth-error": "lifePulse.growthError",
  "growth-skipped": "lifePulse.growthSkipped",
  "rhythm-started": "lifePulse.rhythmStarted",
  "rhythm-done": "lifePulse.rhythmDone",
  "rhythm-error": "lifePulse.rhythmError",
  "rhythm-skipped": "lifePulse.rhythmSkipped",
  "heartbeat-error": "lifePulse.heartbeatError",
  heartbeat: "",
};

const MAX_ACTIVITIES = 20;

interface StreamEventPayload {
  event: string;
  data?: Record<string, unknown>;
  session_key?: string;
}

export function useLifePulse() {
  const [status, setStatus] = useState<LifePulseStatus | null>(null);
  const [activities, setActivities] = useState<LifePulseActivity[]>([]);
  const [lastHeartbeat, setLastHeartbeat] = useState<number>(0);
  const [chatting, setChatting] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<LifePulseStatus>("skilllite_life_pulse_status");
      setStatus(s);
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    refresh();

    const unlistenPulse = listen<PulseEvent>("life-pulse", (ev) => {
      const e = ev.payload;

      if (e.type === "heartbeat") {
        setLastHeartbeat(e.ts);
        return;
      }

      const i18nKey = PULSE_I18N_KEYS[e.type];
      const label =
        e.type === "heartbeat"
          ? ""
          : i18nKey
            ? translate(i18nKey)
            : e.type;
      if (!label) return;

      setActivities((prev) => {
        const next = [{ type: e.type, ts: e.ts, detail: e.detail, label }, ...prev];
        return next.slice(0, MAX_ACTIVITIES);
      });

      if (
        e.type === "growth-done" ||
        e.type === "growth-started" ||
        e.type === "rhythm-done" ||
        e.type === "rhythm-started"
      ) {
        refresh();
      }
    });

    const unlistenChat = listen<StreamEventPayload>("skilllite-event", (ev) => {
      const sk = ev.payload.session_key;
      const current = useSessionStore.getState().currentSessionKey;
      if (sk == null || sk !== current) return;
      const { event } = ev.payload;
      if (event === "text_chunk" || event === "text") {
        setChatting(true);
      } else if (event === "done" || event === "error") {
        setChatting(false);
      }
    });

    return () => {
      unlistenPulse.then((fn) => fn());
      unlistenChat.then((fn) => fn());
    };
  }, [refresh]);

  const toggle = useCallback(
    async (enabled: boolean) => {
      try {
        await invoke("skilllite_life_pulse_toggle", { enabled });
        await refresh();
      } catch (e) {
        useUiToastStore
          .getState()
          .show(
            translate("toast.lifePulseToggleFailed", { err: formatInvokeError(e) }),
            "error"
          );
      }
    },
    [refresh],
  );

  const setWorkspace = useCallback(
    async (ws: string) => {
      try {
        await invoke("skilllite_life_pulse_set_workspace", { workspace: ws });
        await refresh();
      } catch (e) {
        useUiToastStore
          .getState()
          .show(
            translate("toast.lifePulseWsFailed", { err: formatInvokeError(e) }),
            "error"
          );
      }
    },
    [refresh],
  );

  return { status, activities, lastHeartbeat, chatting, toggle, setWorkspace, refresh };
}
