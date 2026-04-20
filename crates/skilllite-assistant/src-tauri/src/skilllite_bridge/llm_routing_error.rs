use serde::Serialize;

/// Structured error kinds used by assistant-side LLM route fallback decisions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmRoutingErrorKind {
    MissingApiKey,
    AuthInvalid,
    PermissionDenied,
    ModelNotFound,
    InvalidRequest,
    RateLimited,
    ProviderUnavailable,
    NetworkTimeout,
    NetworkUnavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmRoutingError {
    pub kind: LlmRoutingErrorKind,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmInvokeResult<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LlmRoutingError>,
}

impl<T> LlmInvokeResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: LlmRoutingError) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(error),
        }
    }
}

impl LlmRoutingError {
    pub fn new(kind: LlmRoutingErrorKind, retryable: bool, message: impl Into<String>) -> Self {
        Self {
            kind,
            retryable,
            message: message.into(),
        }
    }
}

fn classify_http_status(code: u16, msg: &str) -> LlmRoutingError {
    match code {
        401 => LlmRoutingError::new(LlmRoutingErrorKind::AuthInvalid, false, msg),
        403 => LlmRoutingError::new(LlmRoutingErrorKind::PermissionDenied, false, msg),
        404 => LlmRoutingError::new(LlmRoutingErrorKind::ModelNotFound, false, msg),
        408 | 504 => LlmRoutingError::new(LlmRoutingErrorKind::NetworkTimeout, true, msg),
        429 => LlmRoutingError::new(LlmRoutingErrorKind::RateLimited, true, msg),
        500..=599 => LlmRoutingError::new(LlmRoutingErrorKind::ProviderUnavailable, true, msg),
        _ => LlmRoutingError::new(LlmRoutingErrorKind::InvalidRequest, false, msg),
    }
}

fn extract_http_status_code(msg: &str) -> Option<u16> {
    let marker = "HTTP ";
    let idx = msg.find(marker)?;
    let digits: String = msg[idx + marker.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.len() == 3 {
        digits.parse().ok()
    } else {
        None
    }
}

/// Classify a routing-relevant error into a structured kind.
///
/// Decision order:
/// 1. Explicitly-known local validation/configuration messages
/// 2. HTTP status code embedded by provider error formatters
/// 3. Narrow network phrase fallback for legacy/raw message paths
/// 4. Unknown (non-retryable)
pub fn classify_llm_routing_error_message(message: &str) -> LlmRoutingError {
    let msg = message.trim();
    let lower = msg.to_ascii_lowercase();

    if lower.contains("api key not configured")
        || msg.contains("缺少 API key")
        || msg.contains("未配置 API key")
    {
        return LlmRoutingError::new(LlmRoutingErrorKind::MissingApiKey, false, msg);
    }
    if lower.contains("api key 无效")
        || lower.contains("invalid api key")
        || lower.contains("unauthorized")
    {
        return LlmRoutingError::new(LlmRoutingErrorKind::AuthInvalid, false, msg);
    }
    if lower.contains("权限不足") || lower.contains("permission denied") {
        return LlmRoutingError::new(LlmRoutingErrorKind::PermissionDenied, false, msg);
    }
    if lower.contains("unsupported image media_type")
        || lower.contains("not supported")
        || lower.contains("bad request")
    {
        return LlmRoutingError::new(LlmRoutingErrorKind::InvalidRequest, false, msg);
    }
    if let Some(code) = extract_http_status_code(msg) {
        return classify_http_status(code, msg);
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return LlmRoutingError::new(LlmRoutingErrorKind::NetworkTimeout, true, msg);
    }
    if lower.contains("network")
        || lower.contains("econnrefused")
        || lower.contains("econnreset")
        || lower.contains("enotfound")
        || lower.contains("fetch failed")
    {
        return LlmRoutingError::new(LlmRoutingErrorKind::NetworkUnavailable, true, msg);
    }
    LlmRoutingError::new(LlmRoutingErrorKind::Unknown, false, msg)
}

#[cfg(test)]
mod tests {
    use super::{classify_llm_routing_error_message, LlmRoutingErrorKind};

    #[test]
    fn classifies_rate_limit_as_retryable() {
        let err = classify_llm_routing_error_message(
            "LLM API 错误 (HTTP 429): 请求频率超限 (Rate Limit)，请稍后重试",
        );
        assert!(err.retryable);
        assert!(matches!(err.kind, LlmRoutingErrorKind::RateLimited));
    }

    #[test]
    fn classifies_missing_api_key_as_non_retryable() {
        let err =
            classify_llm_routing_error_message("执行 evolution run 失败: 缺少 API key（请配置）");
        assert!(!err.retryable);
        assert!(matches!(err.kind, LlmRoutingErrorKind::MissingApiKey));
    }
}
