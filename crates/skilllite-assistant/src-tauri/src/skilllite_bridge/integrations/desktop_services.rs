//! 引导体检、内置运行时预取、Ollama 探测、`schedule.json` 读写。

use serde::Serialize;
use skilllite_sandbox::{ProvisionRuntimesResult, RuntimeUiSnapshot};
use tauri::Emitter;

/// Requested provider during onboarding health check.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingProvider {
    Api,
    Ollama,
}

/// 反序列化时接受 `apiKey`（Tauri 与 Rust `api_key` 的约定映射）以及历史误用的 `api_key`。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingHealthCheckInput {
    pub workspace: String,
    pub provider: OnboardingProvider,
    #[serde(default, alias = "api_key")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheckItem {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OnboardingHealthCheckResult {
    pub binary: HealthCheckItem,
    pub provider: HealthCheckItem,
    pub workspace: HealthCheckItem,
    pub data_dir: HealthCheckItem,
    pub ok: bool,
}

pub fn run_onboarding_health_check(
    skilllite_path: &std::path::Path,
    workspace: &str,
    provider: OnboardingProvider,
    api_key: Option<&str>,
) -> OnboardingHealthCheckResult {
    let binary = check_bundled_skilllite(skilllite_path);
    let provider = check_provider(provider, api_key);
    let workspace = check_workspace(workspace);
    let data_dir = check_data_dir();
    let ok = binary.ok && provider.ok && workspace.ok && data_dir.ok;
    OnboardingHealthCheckResult {
        binary,
        provider,
        workspace,
        data_dir,
        ok,
    }
}

fn check_bundled_skilllite(skilllite_path: &std::path::Path) -> HealthCheckItem {
    if !skilllite_path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("未找到 SkillLite 二进制：{}", skilllite_path.display()),
        };
    }

    let mut version_cmd = std::process::Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut version_cmd);
    match version_cmd
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => HealthCheckItem {
            ok: true,
            message: format!("内置引擎可用：{}", skilllite_path.display()),
        },
        Ok(status) => HealthCheckItem {
            ok: false,
            message: format!("内置引擎启动失败（状态：{}）", status),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法启动内置引擎：{}", e),
        },
    }
}

fn check_provider(provider: OnboardingProvider, api_key: Option<&str>) -> HealthCheckItem {
    match provider {
        OnboardingProvider::Api => {
            let has_key = api_key.map(|k| !k.trim().is_empty()).unwrap_or(false);
            if has_key {
                HealthCheckItem {
                    ok: true,
                    message: "已填写 API Key，可使用云模型".to_string(),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "尚未填写 API Key".to_string(),
                }
            }
        }
        OnboardingProvider::Ollama => {
            let result = probe_ollama();
            if result.available && result.models.iter().any(|m| !m.contains("embed")) {
                HealthCheckItem {
                    ok: true,
                    message: format!("本机 Ollama 可用，检测到 {} 个模型", result.models.len()),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "未检测到可用的 Ollama 聊天模型".to_string(),
                }
            }
        }
    }
}

fn check_workspace(workspace: &str) -> HealthCheckItem {
    let path = std::path::Path::new(workspace);
    if !path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不存在：{}", path.display()),
        };
    }
    if !path.is_dir() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不是目录：{}", path.display()),
        };
    }

    let probe_dir = path.join(".skilllite");
    match std::fs::create_dir_all(&probe_dir) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("工作区可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("工作区不可写：{}", e),
        },
    }
}

fn check_data_dir() -> HealthCheckItem {
    let path = skilllite_core::paths::data_root();
    match std::fs::create_dir_all(&path) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("数据目录可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法创建数据目录：{}", e),
        },
    }
}

/// Run `skilllite init` in the given directory. Creates .skills and example content.
pub fn init_workspace(dir: &str, skilllite_path: &std::path::Path) -> Result<(), String> {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Err("工作区路径为空".to_string());
    }
    let path = std::path::Path::new(trimmed);
    if !path.is_absolute() {
        return Err(
            "初始化技能需要工作区的绝对路径。请在「设置 → 工作区」点击「浏览」选择项目文件夹；\
             不要将路径设为「.」或相对路径。（桌面应用进程的工作目录常为 /，使用「.」会在 /skills 创建目录并失败。）"
                .to_string(),
        );
    }
    if !path.is_dir() {
        return Err("目录不存在".to_string());
    }
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if path == std::path::Path::new("/") {
        return Err("不能使用根目录 / 作为工作区".to_string());
    }
    let mut cmd = std::process::Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.arg("init").current_dir(&path);
    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite init 失败: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

/// 供设置页展示：本机 `git` 是否在 PATH 中（从 GitHub 等源 `skilllite add` 克隆仓库时需要）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitUiStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_line: Option<String>,
    /// 技术向错误摘要（未找到可执行文件、非零退出等）；用户向说明由前端 i18n 提供。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

pub fn probe_git_status() -> GitUiStatus {
    let mut cmd = std::process::Command::new("git");
    crate::windows_spawn::hide_child_console(&mut cmd);
    match cmd.arg("--version").output() {
        Ok(out) if out.status.success() => {
            let line = String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(std::string::ToString::to_string);
            GitUiStatus {
                available: true,
                version_line: line,
                error_detail: None,
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let hint = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("git exited with status {}", out.status)
            };
            GitUiStatus {
                available: false,
                version_line: None,
                error_detail: Some(hint),
            }
        }
        Err(e) => GitUiStatus {
            available: false,
            version_line: None,
            error_detail: Some(if e.kind() == std::io::ErrorKind::NotFound {
                "git executable not found in PATH".to_string()
            } else {
                format!("failed to run git: {e}")
            }),
        },
    }
}

/// Python/Node 来源探测（系统 PATH vs SkillLite 缓存下载），供左侧栏等 UI 展示。
pub fn probe_runtime_status() -> RuntimeUiSnapshot {
    skilllite_sandbox::probe_runtime_for_ui(None)
}

/// 预下载内置 Python/Node 运行时到缓存目录（`force` 时先删再下）。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvisionProgressPayload {
    /// `"python"` | `"node"`
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
}

fn parse_percent_from_runtime_progress_message(msg: &str) -> Option<u32> {
    for part in msg.split_whitespace() {
        if let Some(n) = part.strip_suffix('%') {
            if let Ok(p) = n.parse::<u32>() {
                return Some(p.min(100));
            }
        }
    }
    None
}

fn emit_runtime_provision_progress(app: &tauri::AppHandle, phase: &'static str, message: &str) {
    let percent = parse_percent_from_runtime_progress_message(message);
    let _ = app.emit(
        "skilllite-runtime-provision-progress",
        RuntimeProvisionProgressPayload {
            phase: phase.to_string(),
            message: message.to_string(),
            percent,
        },
    );
}

/// 无进度事件时使用（测试或脚本）；桌面端请用 [`provision_runtimes_with_emit`]。
#[allow(dead_code)]
pub fn provision_runtimes(python: bool, node: bool, force: bool) -> ProvisionRuntimesResult {
    skilllite_sandbox::provision_runtimes_to_cache(None, python, node, force, None, None)
}

/// 与 [`provision_runtimes`] 相同，但通过 `skilllite-runtime-provision-progress` 事件推送进度文案。
pub fn provision_runtimes_with_emit(
    app: &tauri::AppHandle,
    python: bool,
    node: bool,
    force: bool,
) -> ProvisionRuntimesResult {
    let py_progress = if python {
        let app = app.clone();
        Some(Box::new(move |m: &str| {
            emit_runtime_provision_progress(&app, "python", m);
        }) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };
    let node_progress = if node {
        let app = app.clone();
        Some(Box::new(move |m: &str| {
            emit_runtime_provision_progress(&app, "node", m);
        }) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };
    skilllite_sandbox::provision_runtimes_to_cache(
        None,
        python,
        node,
        force,
        py_progress,
        node_progress,
    )
}

/// Result of probing local Ollama (localhost:11434).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OllamaProbeResult {
    pub available: bool,
    /// All installed model names.
    pub models: Vec<String>,
    /// Whether an embedding-capable model is present (name contains "embed").
    pub has_embedding: bool,
}

/// Probe Ollama at localhost:11434; returns availability, all model names, and embedding support.
pub fn probe_ollama() -> OllamaProbeResult {
    let empty = OllamaProbeResult {
        available: false,
        models: vec![],
        has_embedding: false,
    };
    let body = match ollama_get_tags() {
        Ok(b) => b,
        Err(_) => return empty,
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(j) => j,
        Err(_) => return empty,
    };
    let arr = match json.get("models").and_then(|m| m.as_array()) {
        Some(a) => a,
        None => {
            return OllamaProbeResult {
                available: true,
                models: vec![],
                has_embedding: false,
            }
        }
    };
    let models: Vec<String> = arr
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string())
        .collect();
    let has_embedding = models.iter().any(|n| n.contains("embed"));
    OllamaProbeResult {
        available: true,
        models,
        has_embedding,
    }
}

fn ollama_get_tags() -> Result<String, ()> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = ([127, 0, 0, 1], 11434).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| ())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| ())?;

    let req = b"GET /api/tags HTTP/1.1\r\nHost: localhost:11434\r\nConnection: close\r\n\r\n";
    stream.write_all(req).map_err(|_| ())?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|_| ())?;
    let s = String::from_utf8_lossy(&buf);
    let body = s.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    Ok(body.to_string())
}

/// Read `.skilllite/schedule.json` as pretty JSON; missing file → default `ScheduleFile`.
pub fn read_schedule_json(workspace: &str) -> Result<String, String> {
    use std::path::Path;
    let ws = Path::new(workspace);
    let sched = skilllite_core::schedule::load_schedule(ws)?;
    let file = sched.unwrap_or_default();
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

pub fn write_schedule_json(workspace: &str, json: &str) -> Result<(), String> {
    use std::path::Path;
    let file: skilllite_core::schedule::ScheduleFile =
        serde_json::from_str(json).map_err(|e| format!("schedule.json: {}", e))?;
    skilllite_core::schedule::save_schedule(Path::new(workspace), &file)
}

#[cfg(test)]
mod onboarding_health_check_input_tests {
    use super::OnboardingHealthCheckInput;

    #[test]
    fn deserializes_api_key_camel_case() {
        let j = r#"{"workspace":"/tmp/ws","provider":"api","apiKey":"sk-test"}"#;
        let v: OnboardingHealthCheckInput = serde_json::from_str(j).unwrap();
        assert_eq!(v.workspace, "/tmp/ws");
        assert_eq!(v.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn deserializes_api_key_snake_case_alias() {
        let j = r#"{"workspace":"/tmp/ws","provider":"api","api_key":"sk-legacy"}"#;
        let v: OnboardingHealthCheckInput = serde_json::from_str(j).unwrap();
        assert_eq!(v.api_key.as_deref(), Some("sk-legacy"));
    }
}
