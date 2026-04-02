//! Life Pulse — the heartbeat of the digital life.
//!
//! A background thread that runs while the desktop app is alive (including when
//! minimized to the system tray). Each heartbeat (~30 s) performs lightweight
//! in-process checks for two layers:
//!
//! - **Growth**: runs `skilllite evolution run` as a subprocess when A9 matches the
//!   evolution panel rules: **every `SKILLLITE_EVOLUTION_INTERVAL_SECS`** (default 30 min)
//!   **or** **unprocessed decisions ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`** (default 10).
//!   Chat / agent loop only **records** decisions (monitoring); it does not spawn evolution.
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
    /// Last unix time the **periodic** growth arm fired (`evolution_growth_due`).
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
    pub fn status(&self) -> LifePulseStatus {
        LifePulseStatus {
            enabled: self.enabled.load(Ordering::Relaxed),
            alive: self.alive.load(Ordering::Relaxed),
            growth_running: self.growth_running.load(Ordering::Relaxed),
            rhythm_running: self.rhythm_running.load(Ordering::Relaxed),
            workspace: self.workspace.lock().unwrap_or_else(|e| e.into_inner()).clone(),
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
    let schedule = match skilllite_core::schedule::load_schedule(workspace) {
        Ok(Some(s)) => s,
        _ => return false,
    };
    if !schedule.enabled {
        return false;
    }

    let mut state = skilllite_core::schedule::load_state(workspace);
    skilllite_core::schedule::prepare_state_for_today(&mut state);

    let now = now_unix();
    !skilllite_core::schedule::list_due_job_indices(&schedule, &state, now).is_empty()
}

// ─── Subprocess helpers ─────────────────────────────────────────────────────

fn spawn_growth(
    skilllite_path: &std::path::Path,
    env_pairs: &[(String, String)],
    running: Arc<AtomicBool>,
    app: tauri::AppHandle,
) {
    let path = skilllite_path.to_path_buf();
    let env: Vec<(String, String)> = env_pairs.to_vec();
    std::thread::spawn(move || {
        emit(&app, "growth-started", None);
        let result = Command::new(&path)
            .args(["evolution", "run"])
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
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
    env_pairs: &[(String, String)],
    running: Arc<AtomicBool>,
    app: tauri::AppHandle,
) {
    let path = skilllite_path.to_path_buf();
    let env: Vec<(String, String)> = env_pairs.to_vec();
    std::thread::spawn(move || {
        emit(&app, "rhythm-started", None);
        let result = Command::new(&path)
            .args(["schedule", "tick"])
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
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
    let handle = std::thread::Builder::new()
        .name("life-pulse".into())
        .spawn(move || {
            s.alive.store(true, Ordering::SeqCst);
            eprintln!("[life-pulse] heartbeat started");

            let interval = Duration::from_secs(
                std::env::var("SKILLLITE_HEARTBEAT_INTERVAL_SECS")
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

                // ── Growth ──
                if !s.growth_running.load(Ordering::Relaxed)
                    && skilllite_bridge::evolution_growth_due(
                        &workspace,
                        s.last_periodic_growth_unix.as_ref(),
                    )
                {
                    s.growth_running.store(true, Ordering::SeqCst);
                    spawn_growth(
                        &skilllite_path,
                        &dotenv,
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
                        &dotenv,
                        s.rhythm_running.clone(),
                        app.clone(),
                    );
                }

                emit(&app, "heartbeat", None);
            }

            eprintln!("[life-pulse] heartbeat stopped");
        })
        .expect("failed to spawn life-pulse thread");

    if let Ok(mut guard) = state.thread_handle.lock() {
        *guard = Some(handle);
    }
}

pub fn stop(state: &LifePulseState) {
    state.alive.store(false, Ordering::SeqCst);
}
