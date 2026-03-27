import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

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

const EVENT_LABELS: Record<string, string> = {
  "growth-started": "正在进化…",
  "growth-done": "学到了新东西",
  "growth-error": "进化受阻",
  "growth-skipped": "暂无新领悟",
  "rhythm-started": "开始行动…",
  "rhythm-done": "任务完成",
  "rhythm-error": "任务遇到阻碍",
  "rhythm-skipped": "暂无待办",
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

      const label = EVENT_LABELS[e.type] ?? e.type;
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
      } catch {
        /* ignore */
      }
    },
    [refresh],
  );

  const setWorkspace = useCallback(
    async (ws: string) => {
      try {
        await invoke("skilllite_life_pulse_set_workspace", { workspace: ws });
        await refresh();
      } catch {
        /* ignore */
      }
    },
    [refresh],
  );

  return { status, activities, lastHeartbeat, chatting, toggle, setWorkspace, refresh };
}
