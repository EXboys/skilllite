//! Life Pulse — the heartbeat of the digital life.
//!
//! A background thread that runs while the desktop app is alive (including when
//! minimized to the system tray). Each heartbeat (~30 s) performs lightweight
//! in-process checks for two layers:
//!
//! - **Growth**: runs `skilllite evolution run` when A9 matches workspace `.env`:
//!   **every `SKILLLITE_EVOLUTION_INTERVAL_SECS`** (default 10 min) **or**
//!   **weighted recent signals ≥ `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN`** (default 3) **or**
//!   **raw unprocessed ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`** (default 10) **or** sweep.
//!   Child env = workspace `.env` merged with **assistant Settings** (API key / base / model, etc.),
//!   same rules as chat, via `skilllite_life_pulse_set_llm_overrides` from the UI.
//!   The **agent-rpc** chat subprocess also runs the same A9 timers in-process; `run_evolution`
//!   serializes on `feedback.sqlite` (`SkippedBusy` if both fire).
//! - **Rhythm**: checks `schedule.json` for due jobs and runs
//!   `skilllite schedule tick` as a subprocess when any are due.
//!
//! Heavy work (LLM calls, agent chat) always happens in a child process, keeping
//! the Tauri host lean.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::Emitter;

use crate::skilllite_bridge;

const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 30;

// ─── Public state managed by Tauri ──────────────────────────────────────────

#[derive(Clone)]
pub struct LifePulseState {
    pub enabled: Arc<AtomicBool>,
    alive: Arc<AtomicBool>,
    growth_running: Arc<AtomicBool>,
    rhythm_running: Arc<AtomicBool>,
    thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    workspace: Arc<Mutex<String>>,
    /// LLM-related overrides from the assistant UI (persisted in the webview); merged into child env.
    llm_overrides: Arc<Mutex<Option<skilllite_bridge::ChatConfigOverrides>>>,
    /// Last unix time the **periodic** growth arm advanced (`evolution_growth_due` / `growth_due`).
    last_periodic_growth_unix: Arc<Mutex<Option<i64>>>,
}

impl Default for LifePulseState {
    fn default() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            alive: Arc::new(AtomicBool::new(false)),
            growth_running: Arc::new(AtomicBool::new(false)),
            rhythm_running: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
            workspace: Arc::new(Mutex::new(String::new())),
            llm_overrides: Arc::new(Mutex::new(None)),
            last_periodic_growth_unix: Arc::new(Mutex::new(None)),
        }
    }
}

// ─── Status DTO returned to frontend ────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct LifePulseStatus {
    pub enabled: bool,
    pub alive: bool,
    pub growth_running: bool,
    pub rhythm_running: bool,
    pub workspace: String,
}

impl LifePulseState {
    /// For evolution status UI: periodic arm anchor (same mutex as `evolution_growth_due`).
    #[must_use]
    pub fn periodic_anchor_unix(&self) -> Option<i64> {
        self.last_periodic_growth_unix
            .lock()
            .ok()
            .and_then(|guard| *guard)
    }

    pub fn status(&self) -> LifePulseStatus {
        LifePulseStatus {
            enabled: self.enabled.load(Ordering::Relaxed),
            alive: self.alive.load(Ordering::Relaxed),
            growth_running: self.growth_running.load(Ordering::Relaxed),
            rhythm_running: self.rhythm_running.load(Ordering::Relaxed),
            workspace: self
                .workspace
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        }
    }

    pub fn set_enabled(&self, v: bool) {
        self.enabled.store(v, Ordering::SeqCst);
    }

    pub fn set_workspace(&self, ws: &str) {
        if let Ok(mut guard) = self.workspace.lock() {
            *guard = ws.to_string();
        }
    }

    pub fn set_llm_overrides(&self, cfg: Option<skilllite_bridge::ChatConfigOverrides>) {
        if let Ok(mut guard) = self.llm_overrides.lock() {
            *guard = cfg;
        }
    }
}

// ─── Events emitted to frontend ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct PulseEvent {
    #[serde(rename = "type")]
    kind: String,
    ts: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn emit(app: &tauri::AppHandle, kind: &str, detail: Option<String>) {
    let evt = PulseEvent {
        kind: kind.to_string(),
        ts: now_unix(),
        detail,
    };
    let _ = app.emit("life-pulse", &evt);
}

// ─── Rhythm check (in-process, no LLM) ─────────────────────────────────────

fn check_schedule_due(workspace: &std::path::Path) -> bool {
    let schedule = match skilllite_bridge::local::load_schedule(workspace) {
        Ok(Some(s)) => s,
        _ => return false,
    };
    if !schedule.enabled {
        return false;
    }

    let mut state = skilllite_bridge::local::load_state(workspace);
    skilllite_bridge::local::prepare_state_for_today(&mut state);

    let now = now_unix();
    !skilllite_bridge::local::list_due_job_indices(&schedule, &state, now).is_empty()
}

// ─── Subprocess helpers ─────────────────────────────────────────────────────

fn build_life_pulse_command(
    skilllite_path: &std::path::Path,
    workspace: &str,
    env_pairs: &[(String, String)],
    args: &[&str],
) -> Command {
    let root = skilllite_bridge::find_project_root(workspace);
    let mut cmd = Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.args(args)
        .envs(env_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .current_dir(&root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if std::path::Path::new(workspace).is_absolute() {
        cmd.env(
            skilllite_bridge::local::env_keys::paths::SKILLLITE_WORKSPACE,
            workspace,
        );
    }
    cmd
}

fn spawn_growth(
    skilllite_path: &std::path::Path,
    workspace: &str,
    env_pairs: &[(String, String)],
    running: Arc<AtomicBool>,
    app: tauri::AppHandle,
) {
    let path = skilllite_path.to_path_buf();
    let workspace = workspace.to_string();
    let env: Vec<(String, String)> = env_pairs.to_vec();
    std::thread::spawn(move || {
        emit(&app, "growth-started", None);
        let result = build_life_pulse_command(
            &path,
            &workspace,
            &env,
            &["evolution", "run", "--workspace", &workspace],
        )
        .status();
        running.store(false, Ordering::SeqCst);
        match result {
            Ok(s) if s.success() => emit(&app, "growth-done", None),
            Ok(s) => emit(
                &app,
                "growth-error",
                Some(format!("exit code: {}", s.code().unwrap_or(-1))),
            ),
            Err(e) => emit(&app, "growth-error", Some(e.to_string())),
        }
    });
}

fn spawn_rhythm(
    skilllite_path: &std::path::Path,
    workspace: &str,
    env_pairs: &[(String, String)],
    running: Arc<AtomicBool>,
    app: tauri::AppHandle,
) {
    let path = skilllite_path.to_path_buf();
    let workspace = workspace.to_string();
    let env: Vec<(String, String)> = env_pairs.to_vec();
    std::thread::spawn(move || {
        emit(&app, "rhythm-started", None);
        let result = build_life_pulse_command(
            &path,
            &workspace,
            &env,
            &["schedule", "tick", "--workspace", &workspace],
        )
        .status();
        running.store(false, Ordering::SeqCst);
        match result {
            Ok(s) if s.success() => emit(&app, "rhythm-done", None),
            Ok(s) => emit(
                &app,
                "rhythm-error",
                Some(format!("exit code: {}", s.code().unwrap_or(-1))),
            ),
            Err(e) => emit(&app, "rhythm-error", Some(e.to_string())),
        }
    });
}

// ─── Main heartbeat loop ────────────────────────────────────────────────────

pub fn start(state: LifePulseState, skilllite_path: PathBuf, app: tauri::AppHandle) {
    let s = state.clone();
    let app_on_spawn_failure = app.clone();
    match std::thread::Builder::new()
        .name("life-pulse".into())
        .spawn(move || {
            s.alive.store(true, Ordering::SeqCst);
            eprintln!("[life-pulse] heartbeat started");

            let interval = Duration::from_secs(
                std::env::var(
                    skilllite_bridge::local::env_keys::desktop::SKILLLITE_HEARTBEAT_INTERVAL_SECS,
                )
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SECS),
            );

            loop {
                std::thread::sleep(interval);

                if !s.alive.load(Ordering::SeqCst) {
                    break;
                }
                if !s.enabled.load(Ordering::SeqCst) {
                    emit(&app, "heartbeat", Some("idle".into()));
                    continue;
                }

                let workspace = s
                    .workspace
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if workspace.is_empty() {
                    emit(&app, "heartbeat", Some("no-workspace".into()));
                    continue;
                }

                let dotenv = skilllite_bridge::load_dotenv_for_child(&workspace);
                let overrides = s.llm_overrides.lock().ok().and_then(|g| g.clone());
                let child_env =
                    skilllite_bridge::merge_dotenv_with_chat_overrides(dotenv, overrides.as_ref());

                // ── Growth ──
                if !s.growth_running.load(Ordering::Relaxed)
                    && skilllite_bridge::evolution_growth_due(
                        &workspace,
                        s.last_periodic_growth_unix.as_ref(),
                        overrides.as_ref(),
                        &skilllite_path,
                    )
                {
                    s.growth_running.store(true, Ordering::SeqCst);
                    spawn_growth(
                        &skilllite_path,
                        &workspace,
                        &child_env,
                        s.growth_running.clone(),
                        app.clone(),
                    );
                }

                // ── Rhythm ──
                let ws_path = std::path::Path::new(&workspace);
                if !s.rhythm_running.load(Ordering::Relaxed) && check_schedule_due(ws_path) {
                    s.rhythm_running.store(true, Ordering::SeqCst);
                    spawn_rhythm(
                        &skilllite_path,
                        &workspace,
                        &child_env,
                        s.rhythm_running.clone(),
                        app.clone(),
                    );
                }

                emit(&app, "heartbeat", None);
            }

            eprintln!("[life-pulse] heartbeat stopped");
        }) {
        Ok(handle) => {
            if let Ok(mut guard) = state.thread_handle.lock() {
                *guard = Some(handle);
            }
        }
        Err(e) => {
            eprintln!(
                "[life-pulse] failed to spawn heartbeat thread (growth/rhythm timers inactive): {}",
                e
            );
            emit(
                &app_on_spawn_failure,
                "heartbeat-error",
                Some(format!("failed to spawn heartbeat thread: {}", e)),
            );
        }
    }
}

pub fn stop(state: &LifePulseState) {
    state.alive.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "skilllite_life_pulse_{}_{}_{}",
            std::process::id(),
            unique,
            name
        ));
        std::fs::create_dir_all(&dir).expect("workspace");
        dir
    }

    fn args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn growth_command_targets_life_pulse_workspace() {
        let workspace = temp_workspace("growth");
        let workspace_str = workspace.to_string_lossy().to_string();
        let env = vec![("SKILLLITE_API_KEY".to_string(), "test-key".to_string())];
        let cmd = build_life_pulse_command(
            std::path::Path::new("skilllite"),
            &workspace_str,
            &env,
            &["evolution", "run", "--workspace", &workspace_str],
        );

        let expected_workspace = workspace.canonicalize().expect("canonical workspace");
        assert_eq!(
            cmd.get_current_dir().expect("current dir"),
            expected_workspace.as_path()
        );
        assert_eq!(
            args(&cmd),
            vec![
                "evolution".to_string(),
                "run".to_string(),
                "--workspace".to_string(),
                workspace_str.clone()
            ]
        );
        assert!(cmd.get_envs().any(|(key, value)| {
            key == skilllite_bridge::local::env_keys::paths::SKILLLITE_WORKSPACE
                && value == Some(std::ffi::OsStr::new(&workspace_str))
        }));

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn rhythm_command_targets_life_pulse_workspace() {
        let workspace = temp_workspace("rhythm");
        let workspace_str = workspace.to_string_lossy().to_string();
        let cmd = build_life_pulse_command(
            std::path::Path::new("skilllite"),
            &workspace_str,
            &[],
            &["schedule", "tick", "--workspace", &workspace_str],
        );

        let expected_workspace = workspace.canonicalize().expect("canonical workspace");
        assert_eq!(
            cmd.get_current_dir().expect("current dir"),
            expected_workspace.as_path()
        );
        assert_eq!(
            args(&cmd),
            vec![
                "schedule".to_string(),
                "tick".to_string(),
                "--workspace".to_string(),
                workspace_str
            ]
        );

        let _ = std::fs::remove_dir_all(workspace);
    }
}
