import type { Settings } from "../stores/useSettingsStore";

/**
 * LLM / agent overrides passed from the UI to the Tauri bridge (chat, evolution trigger, Life Pulse).
 * Matches the shape expected by `ChatConfigOverrides` on the Rust side.
 */
export function buildAssistantBridgeConfig(settings: Settings): Record<string, unknown> {
  const swarmUrlTrimmed = settings.swarmUrl?.trim() ?? "";
  const config: Record<string, unknown> = {};
  if (settings.apiKey) config.api_key = settings.apiKey;
  if (settings.model && settings.model !== "gpt-4o") config.model = settings.model;
  if (settings.workspace && settings.workspace !== ".") config.workspace = settings.workspace;
  if (settings.apiBase) config.api_base = settings.apiBase;
  if (settings.sandboxLevel !== 3) config.sandbox_level = settings.sandboxLevel;
  if (settings.swarmEnabled && swarmUrlTrimmed) config.swarm_url = swarmUrlTrimmed;
  if (settings.maxIterations != null && settings.maxIterations > 0) {
    config.max_iterations = settings.maxIterations;
  }
  if (settings.maxToolCallsPerTask != null && settings.maxToolCallsPerTask > 0) {
    config.max_tool_calls_per_task = settings.maxToolCallsPerTask;
  }
  return config;
}
