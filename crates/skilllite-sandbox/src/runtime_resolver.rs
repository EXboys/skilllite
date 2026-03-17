//! RuntimeResolver trait: extension point for new runtimes (Deno, Lua, etc.)
//!
//! Implement this trait to add support for new languages. The sandbox uses
//! the resolver to select the interpreter by language before execution.

use std::path::PathBuf;

use crate::runner::RuntimePaths;

/// Resolved runtime for a given language: interpreter path and optional env vars.
#[derive(Debug, Clone)]
pub struct ResolvedRuntime {
    /// Path to the interpreter executable (e.g. python3, node, deno)
    pub interpreter: PathBuf,
    /// Extra environment variables (e.g. NODE_PATH for Node.js)
    pub extra_env: Vec<(String, String)>,
}

/// Extension point for resolving runtime interpreters by language.
///
/// Implement this trait to add new runtimes (e.g. Deno, Lua). The sandbox
/// calls `resolve(language)` before execution.
pub trait RuntimeResolver: Send + Sync {
    /// Resolve the interpreter path for a given language.
    /// Returns `None` if the language is not supported.
    fn resolve(&self, language: &str) -> Option<ResolvedRuntime>;
}

impl RuntimeResolver for RuntimePaths {
    fn resolve(&self, language: &str) -> Option<ResolvedRuntime> {
        match language {
            "python" => Some(ResolvedRuntime {
                interpreter: self.python.clone(),
                extra_env: Vec::new(),
            }),
            "bash" => {
                // Bash-tool skills (e.g. agent-browser) may use npm CLI — add node_modules/.bin to PATH
                let mut extra_env = Vec::new();
                if let Some(ref nm) = self.node_modules {
                    let bin = nm.join(".bin");
                    if bin.exists() {
                        let bin_str = bin.to_string_lossy().to_string();
                        let sep = if cfg!(windows) { ";" } else { ":" };
                        if let Ok(path) = std::env::var("PATH") {
                            extra_env
                                .push(("PATH".to_string(), format!("{}{}{}", bin_str, sep, path)));
                        } else {
                            extra_env.push(("PATH".to_string(), bin_str));
                        }
                    }
                }
                Some(ResolvedRuntime {
                    interpreter: PathBuf::from("bash"),
                    extra_env,
                })
            }
            "node" => {
                let mut extra_env = Vec::new();
                if let Some(ref node_modules) = self.node_modules {
                    extra_env.push((
                        "NODE_PATH".to_string(),
                        node_modules.to_string_lossy().to_string(),
                    ));
                }
                Some(ResolvedRuntime {
                    interpreter: self.node.clone(),
                    extra_env,
                })
            }
            _ => None,
        }
    }
}
