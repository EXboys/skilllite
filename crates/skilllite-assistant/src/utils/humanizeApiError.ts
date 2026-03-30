import { translate } from "../i18n";

/** 将 agent / LLM 原始错误转为当前界面语言的可读说明（供聊天流 error 事件使用）。 */
export function humanizeApiError(msg: string): string {
  if (
    /certificate|TLS|ssl|rustls|UnknownIssuer|Connection refused|timed out|dns error|proxy/i.test(
      msg,
    )
  ) {
    return `${msg}\n\n${translate("chat.apiError.tlsSuffix")}`;
  }
  if (
    /API Key 无效|API Key 权限|账户余额|请求频率超限|API 端点不存在|服务端错误/.test(msg)
  ) {
    return msg;
  }
  if (/401|[Uu]nauthorized|invalid.api.key/i.test(msg)) {
    return translate("chat.apiError.unauthorized", { msg });
  }
  if (/429|[Rr]ate.limit/i.test(msg)) {
    return translate("chat.apiError.rateLimit", { msg });
  }
  if (/402|[Ii]nsufficient|balance|quota/i.test(msg)) {
    return translate("chat.apiError.quota", { msg });
  }
  return msg;
}
