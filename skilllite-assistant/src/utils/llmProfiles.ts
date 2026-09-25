import type {
  LlmSavedProfile,
  LlmScenarioRouteKey,
  Provider,
} from "../stores/useSettingsStore";

const LLM_SCENARIO_ROUTE_KEYS: readonly LlmScenarioRouteKey[] = [
  "agent",
  "followup",
  "lifePulse",
  "evolution",
] as const;

function normModel(m: string): string {
  return m.trim();
}

function normApiBase(provider: Provider, b: string): string {
  if (provider === "ollama") {
    return b.trim() || "http://localhost:11434/v1";
  }
  return b.trim();
}

export function profileIdentityKey(p: Pick<LlmSavedProfile, "provider" | "model" | "apiBase">): string {
  const m = normModel(p.model);
  const b = normApiBase(p.provider, p.apiBase);
  return `${p.provider}::${m}::${b}`;
}

/**
 * 按 (provider, model, apiBase) 合并更新；不存在则追加并分配新 id。
 */
export function upsertLlmProfile(
  list: LlmSavedProfile[] | undefined,
  entry: { provider: Provider; model: string; apiBase: string; apiKey: string }
): LlmSavedProfile[] {
  const provider = entry.provider;
  const model = normModel(entry.model) || (provider === "ollama" ? "llama3.2" : "gpt-4o");
  const apiBase = normApiBase(provider, entry.apiBase);
  const apiKey =
    provider === "ollama" ? "ollama" : entry.apiKey.trim();

  const candidate: LlmSavedProfile = {
    id: "",
    provider,
    model,
    apiBase,
    apiKey,
  };
  const key = profileIdentityKey(candidate);
  const arr = [...(list ?? [])];
  const idx = arr.findIndex((p) => profileIdentityKey(p) === key);
  if (idx >= 0) {
    const id = arr[idx].id;
    arr[idx] = { ...candidate, id };
    return arr;
  }
  const id =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `p-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  arr.push({ ...candidate, id });
  return arr;
}

/** 按 id 从列表中移除一条已保存配置；id 为空则原样返回副本。 */
export function removeLlmProfileById(
  list: LlmSavedProfile[] | undefined,
  id: string
): LlmSavedProfile[] {
  const tid = id.trim();
  if (!tid) return [...(list ?? [])];
  return (list ?? []).filter((p) => p.id !== tid);
}

/**
 * 切换模型时：优先匹配同 apiBase 的已保存项，否则取同模型的任意一条。
 */
export function findSavedProfileForModel(
  list: LlmSavedProfile[] | undefined,
  provider: Provider,
  model: string,
  preferredApiBase?: string
): LlmSavedProfile | undefined {
  const arr = list ?? [];
  const m = normModel(model);
  if (!m) return undefined;

  if (provider === "api") {
    const pref = preferredApiBase != null ? normApiBase("api", preferredApiBase) : "";
    if (pref) {
      const exact = arr.find(
        (p) =>
          p.provider === "api" &&
          normModel(p.model) === m &&
          normApiBase("api", p.apiBase) === pref
      );
      if (exact) return exact;
    }
    return arr.find((p) => p.provider === "api" && normModel(p.model) === m);
  }

  return arr.find((p) => p.provider === "ollama" && normModel(p.model) === m);
}

/** 聊天区快速切换：API 需非空 key；Ollama 需有模型名。 */
export function listProfilesForQuickSwitch(list: LlmSavedProfile[] | undefined): LlmSavedProfile[] {
  return (list ?? []).filter((p) => {
    if (p.provider === "api") return p.apiKey.trim().length > 0;
    return p.provider === "ollama" && normModel(p.model).length > 0;
  });
}

function pickFallbackProfileFromRemaining(list: LlmSavedProfile[]): LlmSavedProfile | undefined {
  const quick = listProfilesForQuickSwitch(list);
  if (quick.length > 0) return quick[0];
  if (list.length > 0) return list[0];
  return undefined;
}

export function formatProfileShortLabel(p: LlmSavedProfile): string {
  if (p.provider === "ollama") {
    return `${p.model} (Ollama)`;
  }
  const raw = normApiBase("api", p.apiBase);
  if (!raw) return p.model;
  try {
    const u = new URL(raw.includes("://") ? raw : `https://${raw}`);
    return `${p.model} · ${u.host}`;
  } catch {
    const s = raw.length > 28 ? `${raw.slice(0, 28)}…` : raw;
    return `${p.model} · ${s}`;
  }
}

/** 当前设置是否与某条已保存配置完全一致（用于下拉框高亮）。 */
export function matchActiveProfileId(
  list: LlmSavedProfile[] | undefined,
  cur: Pick<LlmSavedProfile, "provider" | "model" | "apiBase" | "apiKey">
): string {
  const key = profileIdentityKey({
    provider: cur.provider,
    model: cur.model,
    apiBase: cur.apiBase,
  });
  for (const p of list ?? []) {
    if (profileIdentityKey(p) === key && p.apiKey === cur.apiKey) {
      return p.id;
    }
  }
  return "";
}

/** 与 `useSettingsStore` 默认 LLM 字段一致：删除全部已保存后避免继续显示已删模型/Key。 */
const DEFAULT_LLM_SESSION: Pick<
  LlmSavedProfile,
  "provider" | "model" | "apiBase" | "apiKey"
> = {
  provider: "api",
  model: "gpt-4o",
  apiKey: "",
  apiBase: "",
};

export type LlmSessionPatch = Pick<
  LlmSavedProfile,
  "provider" | "model" | "apiBase" | "apiKey"
>;

export interface LlmScenarioReferenceCleanup {
  llmScenarioRoutes?: Partial<Record<LlmScenarioRouteKey, string>>;
  llmScenarioFallbacks?: Partial<Record<LlmScenarioRouteKey, string[]>>;
  removedPrimaryRefs: number;
  removedFallbackRefs: number;
  changed: boolean;
}

/**
 * Remove stale scenario route/fallback references that point to missing saved profiles.
 * Keeps only ids that still exist in `list`, preserving order of valid fallback ids.
 */
export function cleanupLlmScenarioProfileReferences(
  list: LlmSavedProfile[] | undefined,
  routes: Partial<Record<LlmScenarioRouteKey, string>> | undefined,
  fallbacks: Partial<Record<LlmScenarioRouteKey, string[]>> | undefined
): LlmScenarioReferenceCleanup {
  const validIds = new Set((list ?? []).map((p) => p.id));
  const nextRoutes: Partial<Record<LlmScenarioRouteKey, string>> = {};
  const nextFallbacks: Partial<Record<LlmScenarioRouteKey, string[]>> = {};
  let removedPrimaryRefs = 0;
  let removedFallbackRefs = 0;

  for (const key of LLM_SCENARIO_ROUTE_KEYS) {
    const routeId = routes?.[key]?.trim() ?? "";
    if (routeId) {
      if (validIds.has(routeId)) {
        nextRoutes[key] = routeId;
      } else {
        removedPrimaryRefs += 1;
      }
    }

    const rawFallbacks = fallbacks?.[key] ?? [];
    const keptFallbacks: string[] = [];
    for (const raw of rawFallbacks) {
      const id = raw?.trim();
      if (!id) continue;
      if (!validIds.has(id)) {
        removedFallbackRefs += 1;
        continue;
      }
      if (id === routeId || keptFallbacks.includes(id)) continue;
      keptFallbacks.push(id);
    }
    if (keptFallbacks.length > 0) {
      nextFallbacks[key] = keptFallbacks;
    }
  }

  const hadRouteKeys = Object.keys(routes ?? {}).length > 0;
  const hadFallbackKeys = Object.keys(fallbacks ?? {}).length > 0;
  const changed =
    removedPrimaryRefs > 0 ||
    removedFallbackRefs > 0 ||
    (hadRouteKeys && Object.keys(nextRoutes).length === 0) ||
    (hadFallbackKeys && Object.keys(nextFallbacks).length === 0);

  return {
    llmScenarioRoutes: Object.keys(nextRoutes).length > 0 ? nextRoutes : undefined,
    llmScenarioFallbacks:
      Object.keys(nextFallbacks).length > 0 ? nextFallbacks : undefined,
    removedPrimaryRefs,
    removedFallbackRefs,
    changed,
  };
}

/**
 * 删除一条已保存配置；若删除的是当前会话正在使用的那条，则切换到剩余列表中的第一条
 * （优先仍可在快捷切换中展示的项），若无剩余则回落到应用默认 LLM 字段。
 */
export function removeLlmProfileWithSessionReselect(
  list: LlmSavedProfile[] | undefined,
  removeId: string,
  current: LlmSessionPatch
): { llmProfiles: LlmSavedProfile[] } & Partial<LlmSessionPatch> {
  const tid = removeId.trim();
  const nextList = removeLlmProfileById(list, removeId);
  if (!tid || matchActiveProfileId(list, current) !== tid) {
    return { llmProfiles: nextList };
  }
  const fallback = pickFallbackProfileFromRemaining(nextList);
  if (fallback) {
    return {
      llmProfiles: nextList,
      provider: fallback.provider,
      model: fallback.model,
      apiBase: fallback.apiBase,
      apiKey: fallback.apiKey,
    };
  }
  return {
    llmProfiles: nextList,
    ...DEFAULT_LLM_SESSION,
  };
}

export function removeLlmProfileWithRoutingCleanup(
  list: LlmSavedProfile[] | undefined,
  removeId: string,
  current: LlmSessionPatch,
  routing: {
    llmScenarioRoutes?: Partial<Record<LlmScenarioRouteKey, string>>;
    llmScenarioFallbacks?: Partial<Record<LlmScenarioRouteKey, string[]>>;
  }
): ({
  llmProfiles: LlmSavedProfile[];
} & Partial<LlmSessionPatch> & {
    llmScenarioRoutes?: Partial<Record<LlmScenarioRouteKey, string>>;
    llmScenarioFallbacks?: Partial<Record<LlmScenarioRouteKey, string[]>>;
  } & Pick<LlmScenarioReferenceCleanup, "removedPrimaryRefs" | "removedFallbackRefs">) {
  const sessionPatch = removeLlmProfileWithSessionReselect(list, removeId, current);
  const cleaned = cleanupLlmScenarioProfileReferences(
    sessionPatch.llmProfiles,
    routing.llmScenarioRoutes,
    routing.llmScenarioFallbacks
  );
  return {
    ...sessionPatch,
    llmScenarioRoutes: cleaned.llmScenarioRoutes,
    llmScenarioFallbacks: cleaned.llmScenarioFallbacks,
    removedPrimaryRefs: cleaned.removedPrimaryRefs,
    removedFallbackRefs: cleaned.removedFallbackRefs,
  };
}

/** 在设置 / 引导流程中：API 模式仅在 key 非空时写入列表；Ollama 始终写入。 */
export function persistCurrentLlmAsProfile(
  list: LlmSavedProfile[] | undefined,
  entry: { provider: Provider; model: string; apiBase: string; apiKey: string }
): LlmSavedProfile[] {
  if (entry.provider === "api" && !entry.apiKey.trim()) {
    return list ?? [];
  }
  return upsertLlmProfile(list, entry);
}
