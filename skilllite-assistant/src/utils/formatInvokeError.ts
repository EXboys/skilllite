/** 将 Tauri invoke / 任意异常转为可读字符串 */
export function formatInvokeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}
