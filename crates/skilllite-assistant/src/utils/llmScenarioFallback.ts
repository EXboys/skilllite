import type { LlmSavedProfile, Settings } from "../stores/useSettingsStore";
import { buildAssistantBridgeConfig } from "./buildAssistantBridgeConfig";
import {
  applyLlmScenarioRoute,
  LLM_ROUTE_SCENARIOS,
  type LlmRouteScenario,
} from "./llmScenarioRouting";

/** Default cooldown after a retryable failure (process-local; cleared on reload). */
export const DEFAULT_FALLBACK_COOLDOWN_MS = 60_000;

const cooldownUntilByProfileId = new Map<string, number>();

/** Test/debug helper: clear the in-memory cooldown table. */
export function resetLlmFallbackCooldown(): void {
  cooldownUntilByProfileId.clear();
}

function isCoolingDown(profileId: string | null, now: number): boolean {
  if (!profileId) return false;
  const until = cooldownUntilByProfileId.get(profileId);
  if (until == null) return false;
  if (until <= now) {
    cooldownUntilByProfileId.delete(profileId);
    return false;
  }
  return true;
}

function markCooldown(profileId: string | null, durationMs: number, now: number): void {
  if (!profileId || durationMs <= 0) return;
  const next = now + durationMs;
  const cur = cooldownUntilByProfileId.get(profileId);
  if (cur == null || cur < next) {
    cooldownUntilByProfileId.set(profileId, next);
  }
}

/** Per-attempt info passed to the invoker. */
export interface ScenarioAttempt {
  /** Resolved profile id; `null` means "use main settings as-is". */
  profileId: string | null;
  /** True when this attempt is using the scenario's primary mapping (or main settings). */
  isPrimary: boolean;
  /** 0-based index inside the candidate chain. */
  index: number;
}

/** Lightweight outcome description (good for logs / future UI). */
export interface ScenarioFallbackInfo<T> {
  result: T;
  scenario: LlmRouteScenario;
  usedProfileId: string | null;
  attempts: number;
  switched: boolean;
}

interface CandidateEntry {
  profileId: string | null;
  isPrimary: boolean;
  /** Final settings to send (overridden when profileId points to a saved profile). */
  settings: Settings;
}

function findProfile(
  list: LlmSavedProfile[] | undefined,
  id: string
): LlmSavedProfile | undefined {
  return list?.find((p) => p.id === id);
}

function applyProfile(settings: Settings, profile: LlmSavedProfile): Settings {
  return {
    ...settings,
    provider: profile.provider,
    model: profile.model,
    apiBase: profile.apiBase,
    apiKey: profile.apiKey,
  };
}

/**
 * Build the ordered candidate chain for a scenario.
 *
 * - Routing disabled or scenario unmapped: a single attempt with main settings.
 * - Routing enabled with mapping: primary (resolved profile or main fallback) followed by configured fallbacks.
 *   Unknown / blank profile ids are skipped. Duplicates are filtered.
 */
export function buildScenarioCandidates(
  settings: Settings,
  scenario: LlmRouteScenario
): CandidateEntry[] {
  const out: CandidateEntry[] = [];
  const seen = new Set<string>();
  const pushProfile = (profileId: string, isPrimary: boolean): void => {
    if (seen.has(profileId)) return;
    const p = findProfile(settings.llmProfiles, profileId);
    if (!p) return;
    seen.add(profileId);
    out.push({ profileId, isPrimary, settings: applyProfile(settings, p) });
  };

  if (!settings.llmScenarioRoutingEnabled) {
    return [{ profileId: null, isPrimary: true, settings }];
  }

  const primaryId = settings.llmScenarioRoutes?.[scenario]?.trim() ?? "";
  if (primaryId) {
    pushProfile(primaryId, true);
  }
  if (out.length === 0) {
    out.push({ profileId: null, isPrimary: true, settings });
  }

  const fallbackIds = settings.llmScenarioFallbacks?.[scenario] ?? [];
  for (const raw of fallbackIds) {
    const id = raw?.trim();
    if (!id) continue;
    pushProfile(id, false);
  }

  return out;
}

const RETRYABLE_PATTERNS: readonly RegExp[] = [
  /\b429\b/i,
  /\brate[\s_-]?limit/i,
  /\b5\d\d\b/,
  /\btime(d)? ?out\b/i,
  /\btimeout\b/i,
  /\bECONNREFUSED\b/i,
  /\bECONNRESET\b/i,
  /\bENOTFOUND\b/i,
  /\bnetwork\b/i,
  /\bfetch failed\b/i,
  /\bservice unavailable\b/i,
  /\bbad gateway\b/i,
  /\bgateway timeout\b/i,
  /\boverloaded\b/i,
  /\bservice is busy\b/i,
];

/**
 * Heuristic check on a raw error from a Tauri `invoke` call.
 *
 * Tauri serializes Rust errors as plain strings, so we match the message text
 * for known transient failure phrases. Conservative by default: unrecognized
 * errors (auth, schema, missing key) are treated as non-retryable so we don't
 * silently switch profiles for genuine misconfiguration.
 */
export function isRetryableLlmError(err: unknown): boolean {
  const msg =
    err == null
      ? ""
      : err instanceof Error
        ? err.message
        : typeof err === "string"
          ? err
          : (() => {
              try {
                return JSON.stringify(err);
              } catch {
                return String(err);
              }
            })();
  if (!msg) return false;
  return RETRYABLE_PATTERNS.some((re) => re.test(msg));
}

export interface RunWithFallbackOptions {
  /** Cooldown applied to a candidate after a retryable failure (ms). */
  cooldownMs?: number;
  /** Optional hook for diagnostics; called on each switch. */
  onSwitch?: (info: { from: ScenarioAttempt; to: ScenarioAttempt; error: unknown }) => void;
}

/**
 * Run `invoker` against the scenario's candidate chain. Stops on first success
 * or first non-retryable error. Cooling-down profiles are skipped.
 *
 * The invoker receives a fully built bridge `config` (already includes the
 * resolved profile fields) plus per-attempt metadata; it should pass `config`
 * straight to the underlying `invoke` call.
 */
export async function runWithScenarioFallback<T>(
  settings: Settings,
  scenario: LlmRouteScenario,
  invoker: (config: Record<string, unknown>, attempt: ScenarioAttempt) => Promise<T>,
  opts: RunWithFallbackOptions = {}
): Promise<ScenarioFallbackInfo<T>> {
  const cooldownMs = opts.cooldownMs ?? DEFAULT_FALLBACK_COOLDOWN_MS;
  const candidates = buildScenarioCandidates(settings, scenario);
  const now = () => Date.now();

  let lastError: unknown = null;
  let lastAttempt: ScenarioAttempt | null = null;
  let attempts = 0;
  let primaryProfileId: string | null = candidates[0]?.profileId ?? null;

  for (let i = 0; i < candidates.length; i++) {
    const cand = candidates[i];
    if (isCoolingDown(cand.profileId, now())) {
      continue;
    }
    const attempt: ScenarioAttempt = {
      profileId: cand.profileId,
      isPrimary: cand.isPrimary,
      index: i,
    };
    attempts += 1;
    try {
      const config = buildAssistantBridgeConfig(cand.settings);
      const result = await invoker(config, attempt);
      return {
        result,
        scenario,
        usedProfileId: cand.profileId,
        attempts,
        switched: !cand.isPrimary || cand.profileId !== primaryProfileId,
      };
    } catch (err) {
      lastError = err;
      if (!isRetryableLlmError(err)) {
        throw err;
      }
      markCooldown(cand.profileId, cooldownMs, now());
      if (opts.onSwitch) {
        const next = candidates[i + 1];
        if (next) {
          opts.onSwitch({
            from: attempt,
            to: { profileId: next.profileId, isPrimary: next.isPrimary, index: i + 1 },
            error: err,
          });
        }
      }
      lastAttempt = attempt;
    }
  }

  if (lastError != null) {
    throw lastError;
  }
  throw new Error(
    `No LLM scenario candidates available for "${scenario}" (lastAttempt=${lastAttempt?.profileId ?? "none"})`
  );
}

/**
 * Convenience for callers that just want a config object derived from the
 * scenario's primary mapping (no fallback). Equivalent to
 * `buildAssistantBridgeConfigForScenario` but kept here so call sites depending
 * on fallbacks import a single module.
 */
export function buildScenarioBridgeConfig(
  settings: Settings,
  scenario: LlmRouteScenario
): Record<string, unknown> {
  return buildAssistantBridgeConfig(applyLlmScenarioRoute(settings, scenario));
}

/** Persist only non-empty fallback ids and drop empties. */
export function normalizeLlmScenarioFallbacks(
  r: Partial<Record<LlmRouteScenario, string[]>> | undefined
): Partial<Record<LlmRouteScenario, string[]>> | undefined {
  if (!r) return undefined;
  const out: Partial<Record<LlmRouteScenario, string[]>> = {};
  for (const k of LLM_ROUTE_SCENARIOS) {
    const arr = r[k];
    if (!arr) continue;
    const cleaned: string[] = [];
    const seen = new Set<string>();
    for (const raw of arr) {
      const v = raw?.trim();
      if (!v || seen.has(v)) continue;
      seen.add(v);
      cleaned.push(v);
    }
    if (cleaned.length > 0) {
      out[k] = cleaned;
    }
  }
  return Object.keys(out).length > 0 ? out : undefined;
}
