//! OpenClaw / ClawHub extensions under `SKILL.md` front matter `metadata`.
//!
//! Canonical field reference:
//! <https://github.com/openclaw/clawhub/blob/main/docs/skill-format.md>
//!
//! SkillLite folds declarative requirements into the Agent Skills `compatibility` string so
//! language, network, capability inference, and dependency heuristics reuse one code path.
//! This module does **not** execute `install` specs (brew/node/go/uv); it only surfaces them textually.

use serde_json::Value;

/// Keys treated as equivalent OpenClaw metadata roots (ClawHub aliases).
const METADATA_ALIAS_KEYS: [&str; 3] = ["openclaw", "clawdbot", "clawdis"];

/// Pick the OpenClaw block to merge: first alias whose value is an object and carries any
/// merge-relevant field; otherwise the first alias that is a non-null object.
fn openclaw_block(metadata: Option<&Value>) -> Option<&Value> {
    let root = metadata?.as_object()?;
    let mut fallback: Option<&Value> = None;

    for key in METADATA_ALIAS_KEYS {
        let Some(v) = root.get(key) else {
            continue;
        };
        if !v.is_object() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(v);
        }
        if block_has_merge_signals(v) {
            return Some(v);
        }
    }

    fallback
}

fn block_has_merge_signals(v: &Value) -> bool {
    v.get("requires").is_some()
        || v.get("install")
            .and_then(|i| i.as_array())
            .is_some_and(|a| !a.is_empty())
        || v.get("primaryEnv").is_some()
        || v.get("os").is_some()
        || v.get("always").is_some()
        || v.get("skillKey").is_some()
}

fn push_str_array_line(adds: &mut Vec<String>, prefix: &str, v: Option<&Value>) {
    let Some(arr) = v.and_then(|x| x.as_array()) else {
        return;
    };
    let items: Vec<&str> = arr.iter().filter_map(|x| x.as_str()).collect();
    if items.is_empty() {
        return;
    }
    adds.push(format!("{}: {}", prefix, items.join(", ")));
}

fn summarize_install_entries(install: &[Value]) -> Option<String> {
    let mut parts = Vec::new();
    for item in install {
        let kind = item
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let detail = match kind {
            "brew" => item
                .get("formula")
                .or_else(|| item.get("package"))
                .and_then(|v| v.as_str()),
            "node" | "go" => item.get("package").and_then(|v| v.as_str()),
            "uv" => item
                .get("package")
                .or_else(|| item.get("formula"))
                .and_then(|v| v.as_str()),
            _ => None,
        };
        if let Some(d) = detail {
            parts.push(format!("{kind}:{d}"));
        } else {
            parts.push(kind.to_string());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("OpenClaw declared installs: {}", parts.join("; ")))
    }
}

fn collect_openclaw_compatibility_fragments(block: &Value) -> Vec<String> {
    let mut adds = Vec::new();

    if let Some(req) = block.get("requires") {
        push_str_array_line(&mut adds, "Requires bins", req.get("bins"));
        push_str_array_line(
            &mut adds,
            "Requires at least one bin",
            req.get("anyBins"),
        );
        push_str_array_line(&mut adds, "Requires env", req.get("env"));
        push_str_array_line(&mut adds, "Requires config", req.get("config"));
    }

    if let Some(pe) = block.get("primaryEnv").and_then(|v| v.as_str()) {
        let pe = pe.trim();
        if !pe.is_empty() {
            adds.push(format!("Primary credential env: {pe}"));
        }
    }

    push_str_array_line(&mut adds, "Requires OS", block.get("os"));

    if let Some(arr) = block.get("install").and_then(|v| v.as_array()) {
        if let Some(s) = summarize_install_entries(arr) {
            adds.push(s);
        }
    }

    if let Some(ak) = block.get("skillKey").and_then(|v| v.as_str()) {
        let ak = ak.trim();
        if !ak.is_empty() {
            adds.push(format!("OpenClaw skillKey: {ak}"));
        }
    }

    if let Some(always) = block.get("always").and_then(|v| v.as_bool()) {
        if always {
            adds.push("OpenClaw: always-on skill".to_string());
        }
    }

    adds
}

/// Merge OpenClaw / ClawHub `metadata` subtree into the Agent Skills `compatibility` string.
pub fn merge_into_compatibility(
    compat: Option<&str>,
    metadata: Option<&Value>,
) -> Option<String> {
    let Some(block) = openclaw_block(metadata) else {
        return compat.map(String::from);
    };

    let adds = collect_openclaw_compatibility_fragments(block);
    if adds.is_empty() {
        return compat.map(String::from);
    }

    let base = compat.unwrap_or("");
    let merged = if base.is_empty() {
        adds.join(". ")
    } else {
        format!("{}. {}", base, adds.join(". "))
    };
    Some(merged)
}

/// Structured view of OpenClaw `metadata.<alias>.install[]` entries.
///
/// Drives SkillLite dependency resolution as a structured signal so we don't have to
/// reverse-parse `compatibility` text. `brew` / `go` / unknown kinds are recorded for
/// observability but **not** auto-installed (would require host package managers).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenClawInstalls {
    /// `kind: node` packages (npm).
    pub node_packages: Vec<String>,
    /// `kind: uv` packages (pip / Python).
    pub python_packages: Vec<String>,
    /// `kind: brew` / `kind: go` package names (informational only).
    pub system_bins: Vec<String>,
    /// Unrecognized `kind` strings — logged so we can grow coverage later.
    pub unsupported_kinds: Vec<String>,
}

impl OpenClawInstalls {
    pub fn is_empty(&self) -> bool {
        self.node_packages.is_empty()
            && self.python_packages.is_empty()
            && self.system_bins.is_empty()
            && self.unsupported_kinds.is_empty()
    }
}

fn install_entry_package_name(item: &Value, kind: &str) -> Option<String> {
    let key_chain: &[&str] = match kind {
        "brew" => &["formula", "package"],
        "node" | "go" => &["package"],
        "uv" => &["package", "formula"],
        _ => &["package", "formula"],
    };
    for k in key_chain {
        if let Some(s) = item.get(*k).and_then(|v| v.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Extract the structured install summary from the front matter `metadata` value.
/// Returns `None` when no usable OpenClaw block exists.
pub fn extract_installs(metadata: Option<&Value>) -> Option<OpenClawInstalls> {
    let block = openclaw_block(metadata)?;
    let arr = block.get("install").and_then(|v| v.as_array())?;

    let mut out = OpenClawInstalls::default();
    for item in arr {
        let kind = item
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let pkg = install_entry_package_name(item, kind);
        match (kind, pkg) {
            ("node", Some(p)) => out.node_packages.push(p),
            ("uv", Some(p)) => out.python_packages.push(p),
            ("brew", Some(p)) | ("go", Some(p)) => out.system_bins.push(p),
            (k, _) if !k.is_empty() && !["node", "uv", "brew", "go"].contains(&k) => {
                out.unsupported_kinds.push(k.to_string());
            }
            _ => {}
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_requires_and_primary_env() {
        let meta = json!({
            "openclaw": {
                "requires": { "bins": ["uv"], "env": ["GEMINI_API_KEY"], "config": ["browser.enabled"] },
                "primaryEnv": "GEMINI_API_KEY"
            }
        });
        let out = merge_into_compatibility(None, Some(&meta));
        let s = out.expect("merged");
        assert!(s.contains("Requires bins: uv"));
        assert!(s.contains("Requires env: GEMINI_API_KEY"));
        assert!(s.contains("Requires config: browser.enabled"));
        assert!(s.contains("Primary credential env: GEMINI_API_KEY"));
    }

    #[test]
    fn clawdbot_alias_used_when_openclaw_absent() {
        let meta = json!({
            "clawdbot": {
                "requires": { "bins": ["jq"], "anyBins": ["gsed", "sed"] }
            }
        });
        let out = merge_into_compatibility(Some("Base"), Some(&meta));
        let s = out.expect("merged");
        assert!(s.starts_with("Base."));
        assert!(s.contains("Requires bins: jq"));
        assert!(s.contains("Requires at least one bin: gsed, sed"));
    }

    #[test]
    fn install_summary_appended() {
        let meta = json!({
            "openclaw": {
                "install": [
                    { "kind": "brew", "formula": "jq", "bins": ["jq"] },
                    { "kind": "node", "package": "typescript", "bins": ["tsc"] }
                ]
            }
        });
        let out = merge_into_compatibility(None, Some(&meta));
        let s = out.expect("merged");
        assert!(s.contains("OpenClaw declared installs: brew:jq; node:typescript"));
    }

    #[test]
    fn extract_installs_classifies_kinds() {
        let meta = json!({
            "openclaw": {
                "install": [
                    { "kind": "node", "package": "openai" },
                    { "kind": "uv", "package": "httpx" },
                    { "kind": "brew", "formula": "jq" },
                    { "kind": "go", "package": "github.com/x/y" },
                    { "kind": "snap", "package": "vlc" },
                    { "kind": "node" },
                    { }
                ]
            }
        });
        let out = extract_installs(Some(&meta)).expect("installs");
        assert_eq!(out.node_packages, vec!["openai".to_string()]);
        assert_eq!(out.python_packages, vec!["httpx".to_string()]);
        assert_eq!(
            out.system_bins,
            vec!["jq".to_string(), "github.com/x/y".to_string()]
        );
        assert_eq!(out.unsupported_kinds, vec!["snap".to_string()]);
    }

    #[test]
    fn extract_installs_returns_none_when_block_absent() {
        let meta = json!({ "version": "1.0" });
        assert!(extract_installs(Some(&meta)).is_none());
    }

    #[test]
    fn openclaw_preferred_over_later_alias() {
        let meta = json!({
            "openclaw": { "requires": { "bins": ["a"] } },
            "clawdbot": { "requires": { "bins": ["b"] } }
        });
        let out = merge_into_compatibility(None, Some(&meta));
        let s = out.expect("merged");
        assert!(s.contains("Requires bins: a"));
        assert!(!s.contains("Requires bins: b"));
    }
}
