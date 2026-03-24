import { create } from "zustand";
import { persist } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";

export interface SessionInfo {
  session_key: string;
  display_name: string;
  updated_at: string;
  message_preview: string | null;
}

interface SessionState {
  currentSessionKey: string;
  sessions: SessionInfo[];
  loadSessions: () => Promise<void>;
  createSession: (name: string) => string;
  switchSession: (key: string) => void;
  renameSession: (key: string, newName: string) => Promise<void>;
  deleteSession: (key: string) => Promise<void>;
}

export const useSessionStore = create<SessionState>()(
  persist(
    (set, get) => ({
      currentSessionKey: "default",
      sessions: [],

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
          set({ sessions: merged });
        } catch {
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

      createSession: (name: string) => {
        const sessionKey = `s-${Date.now().toString(16)}`;
        const now = Math.floor(Date.now() / 1000).toString();
        const session: SessionInfo = {
          session_key: sessionKey,
          display_name: name,
          updated_at: now,
          message_preview: null,
        };

        invoke("skilllite_stop").catch(() => {});
        set((s) => ({
          sessions: [session, ...s.sessions],
          currentSessionKey: sessionKey,
        }));

        invoke("skilllite_create_session", { displayName: name }).catch(
          () => {}
        );

        return sessionKey;
      },

      switchSession: (key: string) => {
        if (get().currentSessionKey !== key) {
          invoke("skilllite_stop").catch(() => {});
          set({ currentSessionKey: key });
        }
      },

      renameSession: async (key: string, newName: string) => {
        set((s) => ({
          sessions: s.sessions.map((session) =>
            session.session_key === key
              ? { ...session, display_name: newName }
              : session
          ),
        }));
        invoke("skilllite_rename_session", {
          sessionKey: key,
          newName: newName,
        }).catch(() => {});
      },

      deleteSession: async (key: string) => {
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
        invoke("skilllite_delete_session", { sessionKey: key }).catch(
          () => {}
        );
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
