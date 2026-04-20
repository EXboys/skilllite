import type { Settings } from "../stores/useSettingsStore";
import { translate } from "../i18n/translate";
import { useUiToastStore } from "../stores/useUiToastStore";
import { formatProfileShortLabel } from "./llmProfiles";
import { type LlmRouteScenario } from "./llmScenarioRouting";
import {
  runWithScenarioFallback,
  type RunWithFallbackOptions,
  type ScenarioAttempt,
  type ScenarioFallbackInfo,
} from "./llmScenarioFallback";

function profileLabel(settings: Settings, attempt: ScenarioAttempt): string {
  if (!attempt.profileId) {
    return translate("llmFallback.mainProfileLabel");
  }
  const p = settings.llmProfiles?.find((x) => x.id === attempt.profileId);
  return p ? formatProfileShortLabel(p) : attempt.profileId;
}

/**
 * Same as `runWithScenarioFallback`, but emits an info toast on automatic
 * profile switching. Intended for foreground call sites (chat/UI) where the
 * user benefits from seeing that the system swapped models on their behalf.
 */
export async function runWithScenarioFallbackNotified<T>(
  settings: Settings,
  scenario: LlmRouteScenario,
  invoker: (config: Record<string, unknown>, attempt: ScenarioAttempt) => Promise<T>,
  opts: RunWithFallbackOptions = {}
): Promise<ScenarioFallbackInfo<T>> {
  const scenarioLabel = translate(`settings.llmScenarioRoute.${scenario}`);
  const merged: RunWithFallbackOptions = {
    ...opts,
    onSwitch: (info) => {
      try {
        useUiToastStore
          .getState()
          .show(
            translate("toast.llmFallbackSwitched", {
              scenario: scenarioLabel,
              from: profileLabel(settings, info.from),
              to: profileLabel(settings, info.to),
            }),
            "info"
          );
      } catch {
        // Toast must never break the request loop.
      }
      opts.onSwitch?.(info);
    },
  };
  return runWithScenarioFallback(settings, scenario, invoker, merged);
}
