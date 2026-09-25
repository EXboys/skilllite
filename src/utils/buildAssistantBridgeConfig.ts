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
  if (
    settings.contextSoftLimitChars != null &&
    Number.isFinite(settings.contextSoftLimitChars)
  ) {
    const n = Math.trunc(settings.contextSoftLimitChars);
    if (n >= 0) {
      config.context_soft_limit_chars = n;
    }
  }
  if (settings.evolutionIntervalSecs != null && settings.evolutionIntervalSecs > 0) {
    config.evolution_interval_secs = settings.evolutionIntervalSecs;
  }
  if (settings.evolutionDecisionThreshold != null && settings.evolutionDecisionThreshold > 0) {
    config.evolution_decision_threshold = settings.evolutionDecisionThreshold;
  }
  if (settings.evoProfile === "demo" || settings.evoProfile === "conservative") {
    config.evo_profile = settings.evoProfile;
  }
  if (
    settings.evoCooldownHours != null &&
    Number.isFinite(settings.evoCooldownHours) &&
    settings.evoCooldownHours >= 0
  ) {
    config.evo_cooldown_hours = settings.evoCooldownHours;
  }
  config.ui_locale = settings.locale === "en" ? "en" : "zh";
  if (settings.mcpServers !== undefined) {
    config.mcp_servers =
      settings.mcpServers.length > 0
        ? settings.mcpServers.map((s) => ({
            id: s.id,
            enabled: s.enabled,
            command: s.command,
            args: s.args,
            ...(s.cwd != null && s.cwd.trim() !== "" ? { cwd: s.cwd.trim() } : {}),
          }))
        : [];
  }
  return config;
}
