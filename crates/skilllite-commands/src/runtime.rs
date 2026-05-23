//! `skilllite runtime` — probe / provision Python & Node for desktop UI (L2 JSON).

use serde::Serialize;
use skilllite_sandbox::{
    probe_runtime_for_ui, provision_runtimes_to_cache, ProvisionRuntimesResult, RuntimeUiSnapshot,
};

use crate::Result;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvisionProgressLine {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
}

fn parse_percent_from_progress_message(msg: &str) -> Option<u32> {
    for part in msg.split_whitespace() {
        if let Some(n) = part.strip_suffix('%') {
            if let Ok(p) = n.parse::<u32>() {
                return Some(p.min(100));
            }
        }
    }
    None
}

fn emit_progress_line(phase: &str, message: &str) {
    let line = RuntimeProvisionProgressLine {
        phase: phase.to_string(),
        message: message.to_string(),
        percent: parse_percent_from_progress_message(message),
    };
    if let Ok(json) = serde_json::to_string(&line) {
        eprintln!("{json}");
    }
}

/// `skilllite runtime probe`
pub fn cmd_probe(json: bool, cache_dir: Option<&str>) -> Result<()> {
    let snap = probe_runtime_for_ui(cache_dir);
    if json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    } else {
        print_runtime_human(&snap);
    }
    Ok(())
}

/// `skilllite runtime provision` — progress JSON lines on stderr when `--json`.
pub fn cmd_provision(
    json: bool,
    cache_dir: Option<&str>,
    download_python: bool,
    download_node: bool,
    force: bool,
) -> Result<()> {
    let py_progress = if download_python && json {
        Some(Box::new(|m: &str| emit_progress_line("python", m)) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };
    let node_progress = if download_node && json {
        Some(Box::new(|m: &str| emit_progress_line("node", m)) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };

    let result = provision_runtimes_to_cache(
        cache_dir,
        download_python,
        download_node,
        force,
        py_progress,
        node_progress,
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_provision_human(&result);
    }
    Ok(())
}

fn print_runtime_human(snap: &RuntimeUiSnapshot) {
    println!("Python: {} ({})", snap.python.label, snap.python.source);
    if let Some(d) = &snap.python.detail {
        println!("  {d}");
    }
    println!("Node: {} ({})", snap.node.label, snap.node.source);
    if let Some(d) = &snap.node.detail {
        println!("  {d}");
    }
    if let Some(root) = &snap.cache_root {
        println!("Cache: {root}");
    }
}

fn print_provision_human(result: &ProvisionRuntimesResult) {
    if result.python.requested {
        println!(
            "Python: {} — {}",
            if result.python.ok { "ok" } else { "failed" },
            result.python.message
        );
    }
    if result.node.requested {
        println!(
            "Node: {} — {}",
            if result.node.ok { "ok" } else { "failed" },
            result.node.message
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_probe_json_roundtrip() {
        let snap = probe_runtime_for_ui(None);
        let v = serde_json::to_value(&snap).expect("serialize");
        assert!(v.get("python").is_some());
        assert!(v.get("node").is_some());
    }

    #[test]
    fn progress_line_parses_percent() {
        assert_eq!(
            parse_percent_from_progress_message("Downloading… 42%"),
            Some(42)
        );
    }
}
