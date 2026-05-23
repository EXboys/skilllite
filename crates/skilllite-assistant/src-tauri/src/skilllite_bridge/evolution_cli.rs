//! Spawn `skilllite` CLI with workspace env merge (L2 bridge for desktop UI).

use std::io::{BufRead, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use skilllite_sandbox::{ProvisionRuntimesResult, RuntimeUiSnapshot};

use super::chat::{merge_dotenv_with_chat_overrides, ChatConfigOverrides};
use super::paths::{find_project_root, load_dotenv_for_child};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProvisionProgressLine {
    phase: String,
    message: String,
    percent: Option<u32>,
}

pub fn spawn_skilllite(
    skilllite_path: &Path,
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
    args: &[&str],
) -> Result<std::process::Output, String> {
    let root = find_project_root(workspace);
    let mut cmd = Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.args(args);
    let env_pairs = merge_dotenv_with_chat_overrides(load_dotenv_for_child(workspace), cfg);
    for (k, v) in env_pairs {
        cmd.env(k, v);
    }
    cmd.current_dir(&root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd.output()
        .map_err(|e| format!("spawn skilllite {}: {}", args.join(" "), e))
}

pub fn spawn_skilllite_json<T: DeserializeOwned>(
    skilllite_path: &Path,
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
    args: &[&str],
) -> Result<T, String> {
    let output = spawn_skilllite(skilllite_path, workspace, cfg, args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "skilllite {} failed (exit {:?}): {}",
            args.join(" "),
            output.status.code(),
            stderr.trim()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "parse skilllite {} JSON: {} (stdout len={})",
            args.join(" "),
            e,
            output.stdout.len()
        )
    })
}

pub fn spawn_skilllite_runtime_probe(
    skilllite_path: &Path,
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
) -> Result<RuntimeUiSnapshot, String> {
    spawn_skilllite_json(skilllite_path, workspace, cfg, &["runtime", "probe", "--json"])
}

pub fn spawn_skilllite_runtime_provision<F>(
    skilllite_path: &Path,
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
    python: bool,
    node: bool,
    force: bool,
    mut on_progress: F,
) -> Result<ProvisionRuntimesResult, String>
where
    F: FnMut(&str, &str, Option<u32>) + Send + 'static,
{
    let root = find_project_root(workspace);
    let mut args: Vec<&str> = vec!["runtime", "provision", "--json"];
    if python {
        args.push("--python");
    }
    if node {
        args.push("--node");
    }
    if force {
        args.push("--force");
    }

    let mut cmd = Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.args(&args);
    let env_pairs = merge_dotenv_with_chat_overrides(load_dotenv_for_child(workspace), cfg);
    for (k, v) in env_pairs {
        cmd.env(k, v);
    }
    cmd.current_dir(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn skilllite {}: {}", args.join(" "), e))?;

    let stderr = child.stderr.take();
    let mut stdout = child.stdout.take();

    let progress_rx = if let Some(stderr) = stderr {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                if let Ok(p) = serde_json::from_str::<RuntimeProvisionProgressLine>(&line) {
                    let _ = tx.send(p);
                }
            }
        });
        Some(rx)
    } else {
        None
    };

    let mut stdout_bytes = Vec::new();
    if let Some(mut out) = stdout.take() {
        out.read_to_end(&mut stdout_bytes)
            .map_err(|e| format!("read skilllite stdout: {}", e))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("wait skilllite {}: {}", args.join(" "), e))?;

    if let Some(rx) = progress_rx {
        while let Ok(p) = rx.try_recv() {
            on_progress(&p.phase, &p.message, p.percent);
        }
    }

    if !status.success() {
        return Err(format!(
            "skilllite {} failed (exit {:?})",
            args.join(" "),
            status.code()
        ));
    }

    serde_json::from_slice(&stdout_bytes).map_err(|e| {
        format!(
            "parse skilllite {} JSON: {} (stdout len={})",
            args.join(" "),
            e,
            stdout_bytes.len()
        )
    })
}
