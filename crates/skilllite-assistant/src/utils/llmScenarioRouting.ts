import type { LlmScenarioRouteKey, Settings } from "../stores/useSettingsStore";
import { buildAssistantBridgeConfig } from "./buildAssistantBridgeConfig";

/** Logical call sites that may use different saved LLM profiles when routing is enabled. */
export type LlmRouteScenario = LlmScenarioRouteKey;

export const LLM_ROUTE_SCENARIOS: readonly LlmRouteScenario[] = [
  "agent",
  "followup",
  "lifePulse",
  "evolution",
] as const;

/**
 * When `llmScenarioRoutingEnabled` is true and the scenario has a valid saved profile id,
 * returns settings with `provider`, `model`, `apiBase`, `apiKey` replaced by that profile.
 * Otherwise returns `settings` unchanged.
 */
export function applyLlmScenarioRoute(settings: Settings, scenario: LlmRouteScenario): Settings {
  if (!settings.llmScenarioRoutingEnabled) {
    return settings;
  }
  const id = settings.llmScenarioRoutes?.[scenario]?.trim();
  if (!id) {
    return settings;
  }
  const profile = settings.llmProfiles?.find((p) => p.id === id);
  if (!profile) {
    return settings;
  }
  return {
    ...settings,
    provider: profile.provider,
    model: profile.model,
    apiBase: profile.apiBase,
    apiKey: profile.apiKey,
  };
}

export function buildAssistantBridgeConfigForScenario(
  settings: Settings,
  scenario: LlmRouteScenario
): Record<string, unknown> {
  return buildAssistantBridgeConfig(applyLlmScenarioRoute(settings, scenario));
}

/** Persist only non-empty profile ids. */
export function normalizeLlmScenarioRoutes(
  r: Partial<Record<LlmRouteScenario, string>> | undefined
): Partial<Record<LlmRouteScenario, string>> | undefined {
  if (!r) return undefined;
  const out: Partial<Record<LlmRouteScenario, string>> = {};
  for (const k of LLM_ROUTE_SCENARIOS) {
    const v = r[k]?.trim();
    if (v) out[k] = v;
  }
  return Object.keys(out).length > 0 ? out : undefined;
}
