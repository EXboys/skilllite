export interface ModelPreset {
  value: string;
  label: string;
  apiBase?: string;
}

export const API_MODEL_PRESETS: ModelPreset[] = [
  { value: "gpt-5.4", label: "GPT-5.4", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-5.4-pro", label: "GPT-5.4 Pro", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-4o", label: "GPT-4o", apiBase: "https://api.openai.com/v1" },
  { value: "gpt-4o-mini", label: "GPT-4o Mini", apiBase: "https://api.openai.com/v1" },
  { value: "claude-opus-4-6", label: "Claude Opus 4.6", apiBase: "https://api.anthropic.com/v1" },
  { value: "claude-sonnet-4-6", label: "Claude Sonnet 4.6", apiBase: "https://api.anthropic.com/v1" },
  { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5", apiBase: "https://api.anthropic.com/v1" },
  {
    value: "gemini-2.5-pro",
    label: "Gemini 2.5 Pro",
    apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/",
  },
  {
    value: "gemini-2.5-flash",
    label: "Gemini 2.5 Flash",
    apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/",
  },
  {
    value: "gemini-2.5-flash-lite",
    label: "Gemini 2.5 Flash-Lite",
    apiBase: "https://generativelanguage.googleapis.com/v1beta/openai/",
  },
  { value: "deepseek-chat", label: "DeepSeek Chat", apiBase: "https://api.deepseek.com/v1" },
  { value: "deepseek-reasoner", label: "DeepSeek Reasoner", apiBase: "https://api.deepseek.com/v1" },
  {
    value: "qwen-plus",
    label: "Qwen Plus",
    apiBase: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  },
  {
    value: "qwen-max",
    label: "Qwen Max",
    apiBase: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  },
  { value: "MiniMax-M2.5", label: "MiniMax M2.5", apiBase: "https://api.minimax.io/v1" },
  { value: "MiniMax-M2.7", label: "MiniMax M2.7", apiBase: "https://api.minimax.io/v1" },
];

/** 内置预设中该 `model` 的默认 API Base；自定义模型名不在列表中则返回 `undefined`。 */
export function presetApiBaseForModelId(modelId: string): string | undefined {
  return API_MODEL_PRESETS.find((p) => p.value === modelId)?.apiBase;
}
