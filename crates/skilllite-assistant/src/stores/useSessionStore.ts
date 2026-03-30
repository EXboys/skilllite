import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";
import { formatInvokeError } from "../utils/formatInvokeError";
import { useUiToastStore } from "./useUiToastStore";

export interface SessionInfo {
  session_key: string;
  display_name: string;
  updated_at: string;
  message_preview: string | null;
}

interface SessionState {
  currentSessionKey: string;
  sessions: SessionInfo[];
  /** 最近一次从磁盘拉取会话列表失败时的说明（不入 persist） */
  sessionsLoadError: string | null;
  loadSessions: () => Promise<void>;
  clearSessionsLoadError: () => void;
  createSession: (name: string) => Promise<string>;
  switchSession: (key: string) => Promise<void>;
  renameSession: (key: string, newName: string) => Promise<void>;
  deleteSession: (key: string) => Promise<void>;
}

export const useSessionStore = create<SessionState>()(
  persist(
    (set, get) => ({
      currentSessionKey: "default",
      sessions: [],
      sessionsLoadError: null,

      clearSessionsLoadError: () => set({ sessionsLoadError: null }),

      loadSessions: async () => {
        try {
          const remote = await invoke<SessionInfo[]>("skilllite_list_sessions");
          const local = get().sessions;
          const remoteKeys = new Set(remote.map((s) => s.session_key));
          const localOnly = local.filter(
            (s) => !remoteKeys.has(s.session_key)
          );
          const merged = [...remote, ...localOnly];
          merged.sort((a, b) => {
            const ta = parseInt(a.updated_at, 10) || 0;
            const tb = parseInt(b.updated_at, 10) || 0;
            return tb - ta;
          });
          set({ sessions: merged, sessionsLoadError: null });
        } catch (e) {
          const msg = formatInvokeError(e);
          set({ sessionsLoadError: msg });
          if (get().sessions.length === 0) {
            set({
              sessions: [
                {
                  session_key: "default",
                  display_name: "默认会话",
                  updated_at: "0",
                  message_preview: null,
                },
              ],
            });
          }
        }
      },

      createSession: async (name: string) => {
        try {
          await invoke("skilllite_stop");
        } catch (e) {
          useUiToastStore
            .getState()
            .show(`停止当前任务失败：${formatInvokeError(e)}`, "error");
        }

        try {
          const session = await invoke<SessionInfo>(
            "skilllite_create_session",
            { displayName: name }
          );
          set((s) => ({
            sessions: [session, ...s.sessions],
            currentSessionKey: session.session_key,
          }));
          return session.session_key;
        } catch (e) {
          const reason = formatInvokeError(e);
          useUiToastStore
            .getState()
            .show(
              `无法在磁盘创建会话（${reason}）。已使用仅本地的临时会话，重启后可能丢失。`,
              "error"
            );
          const fallbackKey = `s-${Date.now().toString(16)}`;
          const now = Math.floor(Date.now() / 1000).toString();
          const session: SessionInfo = {
            session_key: fallbackKey,
            display_name: name,
            updated_at: now,
            message_preview: null,
          };
          set((s) => ({
            sessions: [session, ...s.sessions],
            currentSessionKey: fallbackKey,
          }));
          return fallbackKey;
        }
      },

      switchSession: async (key: string) => {
        if (get().currentSessionKey === key) return;
        try {
          await invoke("skilllite_stop");
        } catch (e) {
          useUiToastStore
            .getState()
            .show(`停止当前任务失败：${formatInvokeError(e)}`, "error");
        }
        set({ currentSessionKey: key });
      },

      renameSession: async (key: string, newName: string) => {
        const prevSessions = get().sessions;
        set((s) => ({
          sessions: s.sessions.map((session) =>
            session.session_key === key
              ? { ...session, display_name: newName }
              : session
          ),
        }));
        try {
          await invoke("skilllite_rename_session", {
            sessionKey: key,
            newName: newName,
          });
        } catch (e) {
          set({ sessions: prevSessions });
          useUiToastStore
            .getState()
            .show(`重命名会话失败：${formatInvokeError(e)}`, "error");
        }
      },

      deleteSession: async (key: string) => {
        const prevSessions = get().sessions;
        const prevKey = get().currentSessionKey;
        set((s) => {
          const newSessions = s.sessions.filter(
            (session) => session.session_key !== key
          );
          return {
            sessions: newSessions,
            currentSessionKey:
              s.currentSessionKey === key ? "default" : s.currentSessionKey,
          };
        });
        try {
          await invoke("skilllite_delete_session", { sessionKey: key });
        } catch (e) {
          set({
            sessions: prevSessions,
            currentSessionKey: prevKey,
          });
          useUiToastStore
            .getState()
            .show(`删除会话失败：${formatInvokeError(e)}`, "error");
        }
      },
    }),
    {
      name: "skilllite-assistant-session",
      partialize: (s) => ({
        currentSessionKey: s.currentSessionKey,
        sessions: s.sessions,
      }),
    }
  )
);
