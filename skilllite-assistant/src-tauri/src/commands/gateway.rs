//! 本机 Gateway 子进程管理与 loopback health 探测（供 WebView 绕过 fetch 限制）。

/// Result of probing `GET …/health` for `skilllite gateway serve` (native HTTP; WebView `fetch` to loopback often fails with "Load failed").
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantGatewayHealthResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub error: Option<String>,
}

fn validate_loopback_gateway_health_url(raw: &str) -> Result<url::Url, String> {
    use url::Host;

    let u = url::Url::parse(raw.trim()).map_err(|e| format!("invalid URL: {e}"))?;
    if u.scheme() != "http" {
        return Err("only http:// loopback URLs are allowed".to_string());
    }
    match u.host() {
        Some(Host::Ipv4(ip)) if ip.is_loopback() => {}
        Some(Host::Ipv6(ip)) if ip.is_loopback() => {}
        Some(Host::Domain(d)) if d.eq_ignore_ascii_case("localhost") => {}
        _ => {
            return Err("host must be loopback (127.0.0.1, ::1, localhost)".to_string());
        }
    }
    if !u.path().ends_with("/health") {
        return Err("path must end with /health".to_string());
    }
    Ok(u)
}

#[tauri::command]
pub fn assistant_gateway_health_probe(url: String) -> AssistantGatewayHealthResult {
    let parsed = match validate_loopback_gateway_health_url(&url) {
        Ok(u) => u,
        Err(e) => {
            return AssistantGatewayHealthResult {
                ok: false,
                status: None,
                error: Some(e),
            };
        }
    };
    let url = parsed.as_str();
    let resp = match ureq::get(url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return AssistantGatewayHealthResult {
                ok: false,
                status: Some(code),
                error: Some(format!("HTTP {code}")),
            };
        }
        Err(e) => {
            return AssistantGatewayHealthResult {
                ok: false,
                status: None,
                error: Some(e.to_string()),
            };
        }
    };
    let status = resp.status();
    if status != 200 {
        return AssistantGatewayHealthResult {
            ok: false,
            status: Some(status),
            error: Some(format!("HTTP {status}")),
        };
    }
    let body: serde_json::Value = match resp.into_json() {
        Ok(v) => v,
        Err(e) => {
            return AssistantGatewayHealthResult {
                ok: false,
                status: Some(status),
                error: Some(format!("invalid JSON: {e}")),
            };
        }
    };
    let body_ok = body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    AssistantGatewayHealthResult {
        ok: body_ok,
        status: Some(status),
        error: if body_ok {
            None
        } else {
            Some("response body ok is not true".to_string())
        },
    }
}

#[tauri::command]
pub fn assistant_gateway_status(
    gateway_state: tauri::State<'_, crate::gateway_manager::GatewayProcessState>,
    request: crate::gateway_manager::GatewayStatusRequest,
) -> Result<crate::gateway_manager::GatewayManagedStatus, String> {
    crate::gateway_manager::status_gateway(&gateway_state, request)
}

#[tauri::command]
pub fn assistant_gateway_start(
    app: tauri::AppHandle,
    gateway_state: tauri::State<'_, crate::gateway_manager::GatewayProcessState>,
    request: crate::gateway_manager::GatewayStartRequest,
) -> Result<crate::gateway_manager::GatewayManagedStatus, String> {
    crate::gateway_manager::start_gateway(&app, &gateway_state, request)
}

#[tauri::command]
pub fn assistant_gateway_stop(
    gateway_state: tauri::State<'_, crate::gateway_manager::GatewayProcessState>,
) -> Result<crate::gateway_manager::GatewayManagedStatus, String> {
    crate::gateway_manager::stop_gateway(&gateway_state)
}
