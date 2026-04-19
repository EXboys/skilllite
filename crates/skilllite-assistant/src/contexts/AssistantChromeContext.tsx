import { createContext, useContext, useMemo, type ReactNode } from "react";

export type AssistantSettingsTabId =
  | "llm"
  | "workspace"
  | "environment"
  | "agent"
  | "evolution"
  | "schedule"
  | "uninstall";

type AssistantChromeValue = {
  openSettingsToTab: (tab: AssistantSettingsTabId) => void;
};

const AssistantChromeContext = createContext<AssistantChromeValue | null>(null);

export function AssistantChromeProvider({
  children,
  onOpenSettingsToTab,
}: {
  children: ReactNode;
  onOpenSettingsToTab: (tab: AssistantSettingsTabId) => void;
}) {
  const value = useMemo(
    () => ({ openSettingsToTab: onOpenSettingsToTab }),
    [onOpenSettingsToTab]
  );
  return (
    <AssistantChromeContext.Provider value={value}>
      {children}
    </AssistantChromeContext.Provider>
  );
}

export function useAssistantChrome(): AssistantChromeValue {
  const v = useContext(AssistantChromeContext);
  if (!v) {
    throw new Error("useAssistantChrome must be used within AssistantChromeProvider");
  }
  return v;
}
