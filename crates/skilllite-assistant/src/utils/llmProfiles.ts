import type { LlmSavedProfile, Provider } from "../stores/useSettingsStore";

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
