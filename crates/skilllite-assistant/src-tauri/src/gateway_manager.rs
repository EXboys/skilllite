use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge;

const DEFAULT_GATEWAY_BIND: &str = "127.0.0.1:8787";
const RECENT_GATEWAY_LOG_LIMIT: usize = 20;

#[derive(Default)]
struct GatewayRuntime {
    child: Option<Child>,
    bind: Option<String>,
    started_at_ms: Option<u64>,
    last_exit_code: Option<i32>,
    last_error: Option<String>,
    recent_logs: VecDeque<String>,
}

#[derive(Default, Clone)]
pub struct GatewayProcessState(pub Arc<Mutex<GatewayRuntime>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStartRequest {
    pub workspace: Option<String>,
    pub bind: String,
    pub token: Option<String>,
    pub artifact_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayStatusRequest {
    pub bind: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GatewayProcessSource {
    None,
    Managed,
    External,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayManagedStatus {
    pub running: bool,
    pub source: GatewayProcessSource,
    pub pid: Option<u32>,
    pub bind: Option<String>,
    pub started_at_ms: Option<u64>,
    pub last_exit_code: Option<i32>,
    pub last_error: Option<String>,
    pub recent_log: Option<String>,
}

pub fn status_gateway(
    state: &GatewayProcessState,
    request: GatewayStatusRequest,
) -> Result<GatewayManagedStatus, String> {
    let requested_bind = normalize_bind(&request.bind);
    let mut runtime = state
        .0
        .lock()
        .map_err(|_| "GatewayProcessState lock poisoned".to_string())?;
    refresh_runtime_status(&mut runtime);
    if runtime.child.is_some() {
        return Ok(snapshot_runtime(&runtime));
    }
    if probe_gateway_health(&requested_bind) {
        return Ok(external_status(&requested_bind));
    }
    Ok(stopped_status(&runtime, &requested_bind))
}

pub fn start_gateway(
    app: &tauri::AppHandle,
    state: &GatewayProcessState,
    request: GatewayStartRequest,
) -> Result<GatewayManagedStatus, String> {
    let bind = normalize_bind(&request.bind);
    let token = normalize_optional_cli_value(request.token.as_deref());
    let artifact_dir = normalize_optional_cli_value(request.artifact_dir.as_deref());
    let working_dir = resolve_gateway_working_dir(request.workspace.as_deref())?;
    let skilllite_path = skilllite_bridge::resolve_skilllite_path_app(app);

    {
        let mut runtime = state
            .0
            .lock()
            .map_err(|_| "GatewayProcessState lock poisoned".to_string())?;
        refresh_runtime_status(&mut runtime);
        if runtime.child.is_some() {
            return Err("Gateway is already running in this desktop app".to_string());
        }
    }

    if probe_gateway_health(&bind) {
        return Ok(external_status(&bind));
    }

    let mut cmd = Command::new(&skilllite_path);
    cmd.args(build_gateway_cli_args(&bind, token.as_deref(), artifact_dir.as_deref()))
        .env("SKILLLITE_GATEWAY_SERVE_ALLOW", "1")
        .current_dir(&working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::windows_spawn::hide_child_console(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Failed to start gateway with {} from {}: {}",
            skilllite_path.display(),
            working_dir.display(),
            e
        )
    })?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture gateway stderr".to_string())?;

    {
        let mut runtime = state
            .0
            .lock()
            .map_err(|_| "GatewayProcessState lock poisoned".to_string())?;
        runtime.child = Some(child);
        runtime.bind = Some(bind.clone());
        runtime.started_at_ms = Some(now_ms());
        runtime.last_exit_code = None;
        runtime.last_error = None;
        runtime.recent_logs.clear();
    }

    spawn_gateway_stderr_reader(stderr, state.clone());
    thread::sleep(Duration::from_millis(200));

    let status = status_gateway(
        state,
        GatewayStatusRequest {
            bind: bind.clone(),
        },
    )?;
    if status.running {
        Ok(status)
    } else {
        Err(
            status.last_error.unwrap_or_else(|| {
                "Gateway exited immediately after the desktop app tried to start it".to_string()
            }),
        )
    }
}

pub fn stop_gateway(state: &GatewayProcessState) -> Result<GatewayManagedStatus, String> {
    let child = {
        let mut runtime = state
            .0
            .lock()
            .map_err(|_| "GatewayProcessState lock poisoned".to_string())?;
        refresh_runtime_status(&mut runtime);
        runtime.started_at_ms = None;
        runtime.last_exit_code = None;
        runtime.last_error = None;
        runtime.child.take()
    };

    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }

    Ok(GatewayManagedStatus {
        running: false,
        source: GatewayProcessSource::None,
        pid: None,
        bind: None,
        started_at_ms: None,
        last_exit_code: None,
        last_error: None,
        recent_log: None,
    })
}

pub fn cleanup_gateway_process(state: &GatewayProcessState) {
    let child = state.0.lock().ok().and_then(|mut runtime| runtime.child.take());
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn_gateway_stderr_reader(stderr: std::process::ChildStderr, state: GatewayProcessState) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if let Ok(mut runtime) = state.0.lock() {
                        push_recent_log(&mut runtime, line);
                    }
                }
                Err(err) => {
                    if let Ok(mut runtime) = state.0.lock() {
                        let msg = format!("gateway stderr read failed: {err}");
                        push_recent_log(&mut runtime, msg.clone());
                        runtime.last_error = Some(msg);
                    }
                    break;
                }
            }
        }
    });
}

fn resolve_gateway_working_dir(workspace: Option<&str>) -> Result<PathBuf, String> {
    let raw = workspace.unwrap_or("").trim();
    if raw.is_empty() || raw == "." {
        return skilllite_bridge::default_writable_workspace_dir().map(PathBuf::from);
    }

    let path = PathBuf::from(raw);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!(
            "Gateway workspace directory does not exist: {}",
            path.display()
        ))
    }
}

fn build_gateway_cli_args(
    bind: &str,
    token: Option<&str>,
    artifact_dir: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "gateway".to_string(),
        "serve".to_string(),
        "--bind".to_string(),
        bind.to_string(),
    ];
    if let Some(token) = token {
        args.push("--token".to_string());
        args.push(token.to_string());
    }
    if let Some(dir) = artifact_dir {
        args.push("--artifact-dir".to_string());
        args.push(dir.to_string());
    }
    args
}

fn normalize_bind(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_GATEWAY_BIND.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_optional_cli_value(raw: Option<&str>) -> Option<String> {
    raw.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn refresh_runtime_status(runtime: &mut GatewayRuntime) {
    let exit = match runtime.child.as_mut() {
        Some(child) => match child.try_wait() {
            Ok(Some(status)) => Some(status),
            Ok(None) => None,
            Err(err) => {
                runtime.last_error = Some(format!("Failed to inspect gateway process: {err}"));
                None
            }
        },
        None => None,
    };

    if let Some(status) = exit {
        runtime.child = None;
        runtime.started_at_ms = None;
        runtime.last_exit_code = status.code();
        if runtime.last_error.is_none() {
            let fallback = if let Some(line) = runtime.recent_logs.back() {
                line.clone()
            } else {
                match status.code() {
                    Some(code) => format!("Gateway exited with status code {code}"),
                    None => "Gateway exited before the desktop app could inspect a status code"
                        .to_string(),
                }
            };
            runtime.last_error = Some(fallback);
        }
    }
}

fn snapshot_runtime(runtime: &GatewayRuntime) -> GatewayManagedStatus {
    GatewayManagedStatus {
        running: runtime.child.is_some(),
        source: if runtime.child.is_some() {
            GatewayProcessSource::Managed
        } else {
            GatewayProcessSource::None
        },
        pid: runtime.child.as_ref().map(std::process::Child::id),
        bind: runtime.bind.clone(),
        started_at_ms: runtime.started_at_ms,
        last_exit_code: runtime.last_exit_code,
        last_error: runtime.last_error.clone(),
        recent_log: runtime.recent_logs.back().cloned(),
    }
}

fn stopped_status(runtime: &GatewayRuntime, requested_bind: &str) -> GatewayManagedStatus {
    GatewayManagedStatus {
        running: false,
        source: GatewayProcessSource::None,
        pid: None,
        bind: Some(requested_bind.to_string()),
        started_at_ms: None,
        last_exit_code: runtime.last_exit_code,
        last_error: runtime.last_error.clone(),
        recent_log: runtime.recent_logs.back().cloned(),
    }
}

fn external_status(bind: &str) -> GatewayManagedStatus {
    GatewayManagedStatus {
        running: true,
        source: GatewayProcessSource::External,
        pid: None,
        bind: Some(bind.to_string()),
        started_at_ms: None,
        last_exit_code: None,
        last_error: None,
        recent_log: None,
    }
}

fn push_recent_log(runtime: &mut GatewayRuntime, line: String) {
    if runtime.recent_logs.len() >= RECENT_GATEWAY_LOG_LIMIT {
        runtime.recent_logs.pop_front();
    }
    runtime.recent_logs.push_back(line);
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_millis().try_into().unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

fn probe_gateway_health(bind: &str) -> bool {
    let Some(url) = gateway_health_url(bind) else {
        return false;
    };
    let resp = match ureq::get(&url)
        .timeout(std::time::Duration::from_millis(800))
        .call()
    {
        Ok(resp) => resp,
        Err(_) => return false,
    };
    if resp.status() != 200 {
        return false;
    }
    match resp.into_json::<serde_json::Value>() {
        Ok(body) => body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        Err(_) => false,
    }
}

fn gateway_health_url(bind: &str) -> Option<String> {
    let trimmed = bind.trim();
    let (host, port) = trimmed.rsplit_once(':')?;
    let probe_host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
    Some(format!("http://{probe_host}:{port}/health"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_gateway_cli_args_includes_optional_flags() {
        let args = build_gateway_cli_args("127.0.0.1:8787", Some("secret"), Some("./.skilllite"));
        assert_eq!(
            args,
            vec![
                "gateway".to_string(),
                "serve".to_string(),
                "--bind".to_string(),
                "127.0.0.1:8787".to_string(),
                "--token".to_string(),
                "secret".to_string(),
                "--artifact-dir".to_string(),
                "./.skilllite".to_string(),
            ]
        );
    }

    #[test]
    fn build_gateway_cli_args_omits_blank_optional_flags() {
        let args = build_gateway_cli_args(
            &normalize_bind(""),
            normalize_optional_cli_value(Some("   ")).as_deref(),
            normalize_optional_cli_value(Some("")).as_deref(),
        );
        assert_eq!(
            args,
            vec![
                "gateway".to_string(),
                "serve".to_string(),
                "--bind".to_string(),
                "127.0.0.1:8787".to_string(),
            ]
        );
    }

    #[test]
    fn gateway_health_url_rewrites_unspecified_ipv4_bind() {
        assert_eq!(
            gateway_health_url("0.0.0.0:8787").as_deref(),
            Some("http://127.0.0.1:8787/health")
        );
    }

    #[test]
    fn gateway_health_url_preserves_loopback_bind() {
        assert_eq!(
            gateway_health_url("127.0.0.1:8787").as_deref(),
            Some("http://127.0.0.1:8787/health")
        );
    }
}
