//! Swarm TaskExecutor — execute NodeTask locally via agent when swarm routes to self.
//!
//! Used when `skilllite swarm` receives a NodeTask and routing decides Local.
//! Uses the swarm's `--skills-dir` so each node loads its own skills (not workspace auto-discovery).
//!
//! Security: local execution uses the **node** workspace (skills-dir parent, else cwd),
//! never a client-supplied `NodeTask.context.workspace` filesystem root. That field may
//! carry an originating node id for P2P and must not retarget `write_file` containment.

#[cfg(feature = "agent")]
use std::path::{Path, PathBuf};

#[cfg(feature = "agent")]
use skilllite_core::protocol::{NodeResult, NodeTask};
#[cfg(feature = "agent")]
use skilllite_swarm::TaskExecutor;

#[cfg(feature = "agent")]
/// Executor that runs tasks via skilllite_agent, using the swarm's --skills-dir.
#[derive(Debug)]
pub struct AgentTaskExecutor {
    /// Skill directories to load (from --skills-dir). When None, agent auto-discovers from workspace.
    pub skill_dirs: Option<Vec<String>>,
}

#[cfg(feature = "agent")]
impl AgentTaskExecutor {
    pub fn new(skill_dirs: Option<Vec<String>>) -> Self {
        Self { skill_dirs }
    }

    /// Resolve the filesystem workspace used for local swarm task execution.
    ///
    /// Prefers the parent of the first `--skills-dir` entry (project root). Falls back
    /// to the process current directory. Never trusts `task.context.workspace`.
    pub(crate) fn resolve_local_workspace(skill_dirs: Option<&[String]>) -> PathBuf {
        if let Some(dirs) = skill_dirs {
            if let Some(first) = dirs.first() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    let path = Path::new(trimmed);
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() {
                            return parent.to_path_buf();
                        }
                    }
                    // Bare filename / relative single-segment skill dir → use cwd.
                }
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

#[cfg(feature = "agent")]
impl TaskExecutor for AgentTaskExecutor {
    fn execute(
        &self,
        task: NodeTask,
    ) -> Result<NodeResult, Box<dyn std::error::Error + Send + Sync>> {
        // Run in a separate thread to avoid "Cannot start a runtime from within a runtime":
        // handle_task runs on axum's tokio runtime; block_on would nest runtimes.
        let task = task.clone();
        let skill_dirs = self.skill_dirs.clone();
        let handle = std::thread::spawn(move || {
            let workspace = AgentTaskExecutor::resolve_local_workspace(skill_dirs.as_deref());
            if !task.context.workspace.is_empty()
                && task.context.workspace != workspace.to_string_lossy()
            {
                tracing::warn!(
                    task_id = %task.id,
                    requested_workspace = %task.context.workspace,
                    local_workspace = %workspace.display(),
                    "Ignoring client-supplied swarm task workspace; using node workspace"
                );
            }
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
            let result = rt.block_on(skilllite_agent::chat::run_single_task(
                &workspace.to_string_lossy(),
                &task.context.session_key,
                &task.description,
                skill_dirs.as_deref(),
            ))?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(result.to_node_result(&task.id))
        });
        handle.join().map_err(|e| {
            Box::new(std::io::Error::other(format!(
                "Agent execution thread panicked: {:?}",
                e
            ))) as Box<dyn std::error::Error + Send + Sync>
        })?
    }
}

#[cfg(all(test, feature = "agent"))]
mod tests {
    use super::*;

    #[test]
    fn resolve_local_workspace_uses_skills_dir_parent() {
        let dirs = vec!["/home/user/project/skills".to_string()];
        assert_eq!(
            AgentTaskExecutor::resolve_local_workspace(Some(dirs.as_slice())),
            PathBuf::from("/home/user/project")
        );
    }

    #[test]
    fn resolve_local_workspace_ignores_empty_skills_dir_list() {
        let dirs: Vec<String> = vec![];
        let resolved = AgentTaskExecutor::resolve_local_workspace(Some(dirs.as_slice()));
        // Falls back to cwd — just assert it is absolute-ish / non-empty.
        assert!(!resolved.as_os_str().is_empty());
    }

    #[test]
    fn resolve_local_workspace_dot_skills_parent_is_project() {
        let dirs = vec!["/tmp/ws/.skills".to_string()];
        assert_eq!(
            AgentTaskExecutor::resolve_local_workspace(Some(dirs.as_slice())),
            PathBuf::from("/tmp/ws")
        );
    }
}
