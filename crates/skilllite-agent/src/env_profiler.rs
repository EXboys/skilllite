//! Safe environment profiler for planning readiness.
//!
//! This module performs low-risk local checks only:
//! - fixed tool allowlist
//! - read-only version probes (`--version`)
//! - no privilege escalation

use std::process::Command;

const TOOL_ALLOWLIST: [&str; 6] = ["git", "python", "node", "npm", "pip", "cargo"];

#[derive(Debug, Clone)]
pub struct ToolProbe {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EnvProfile {
    pub os: String,
    pub arch: String,
    pub shell: Option<String>,
    pub tools: Vec<ToolProbe>,
}

impl EnvProfile {
    pub fn missing_tools(&self) -> Vec<String> {
        self.tools
            .iter()
            .filter_map(|t| {
                if t.available {
                    None
                } else {
                    Some(t.name.clone())
                }
            })
            .collect()
    }

    pub fn to_planning_block(&self) -> String {
        let mut lines = vec![
            "## Environment Profile (safe runtime readiness checks)".to_string(),
            format!(
                "- **Runtime**: os={}, arch={}, shell={}",
                self.os,
                self.arch,
                self.shell.as_deref().unwrap_or("unknown")
            ),
            "- **Tool availability**:".to_string(),
        ];

        for tool in &self.tools {
            let status = if tool.available {
                "available"
            } else {
                "missing"
            };
            let version = tool.version.as_deref().unwrap_or("n/a");
            lines.push(format!(
                "  - {}: {} (version: {})",
                tool.name, status, version
            ));
        }

        let missing = self.missing_tools();
        if !missing.is_empty() {
            lines.push(format!(
                "- **Missing critical tools**: {}",
                missing.join(", ")
            ));
        }
        lines.push(String::new());
        lines.push(
            "Use this profile to avoid plans that require unavailable local tooling.".to_string(),
        );
        lines.join("\n")
    }
}

fn truncate_line(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

fn first_non_empty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn probe_tool(tool: &str) -> ToolProbe {
    let mut c = Command::new(tool);
    c.arg("--version");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    match c.output() {
        Ok(output) => {
            let raw = if !output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                String::from_utf8_lossy(&output.stderr).to_string()
            };
            let version = first_non_empty_line(&raw).map(|line| truncate_line(&line, 120));
            ToolProbe {
                name: tool.to_string(),
                available: output.status.success(),
                version,
            }
        }
        Err(_) => ToolProbe {
            name: tool.to_string(),
            available: false,
            version: None,
        },
    }
}

pub fn collect_safe_env_profile() -> EnvProfile {
    let shell = std::env::var("SHELL")
        .ok()
        .or_else(|| std::env::var("COMSPEC").ok());
    let tools = TOOL_ALLOWLIST.iter().map(|name| probe_tool(name)).collect();
    EnvProfile {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        shell,
        tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_profile_contains_allowlist_tools() {
        let profile = collect_safe_env_profile();
        let names: Vec<&str> = profile.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["git", "python", "node", "npm", "pip", "cargo"]);
        assert!(!profile.os.is_empty());
        assert!(!profile.arch.is_empty());
    }

    #[test]
    fn planning_block_mentions_missing_tools_when_any() {
        let profile = EnvProfile {
            os: "test-os".to_string(),
            arch: "x86_64".to_string(),
            shell: Some("/bin/sh".to_string()),
            tools: vec![
                ToolProbe {
                    name: "git".to_string(),
                    available: true,
                    version: Some("git version x".to_string()),
                },
                ToolProbe {
                    name: "npm".to_string(),
                    available: false,
                    version: None,
                },
            ],
        };
        let block = profile.to_planning_block();
        assert!(block.contains("Environment Profile"));
        assert!(block.contains("Missing critical tools"));
        assert!(block.contains("npm"));
    }
}
