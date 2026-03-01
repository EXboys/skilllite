//! Swarm TaskExecutor — execute NodeTask locally via agent when swarm routes to self.
//!
//! Used when `skilllite swarm` receives a NodeTask and routing decides Local.
//! Uses the swarm's `--skills-dir` so each node loads its own skills (not workspace auto-discovery).

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
}

#[cfg(feature = "agent")]
impl TaskExecutor for AgentTaskExecutor {
    fn execute(&self, task: NodeTask) -> Result<NodeResult, Box<dyn std::error::Error + Send + Sync>> {
        // Run in a separate thread to avoid "Cannot start a runtime from within a runtime":
        // handle_task runs on axum's tokio runtime; block_on would nest runtimes.
        let task = task.clone();
        let skill_dirs = self.skill_dirs.clone();
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
            let result = rt.block_on(skilllite_agent::chat::run_single_task(
                &task.context.workspace,
                &task.context.session_key,
                &task.description,
                skill_dirs.as_deref(),
            ))?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(result.to_node_result(&task.id))
        });
        handle.join().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Agent execution thread panicked: {:?}", e),
            )) as Box<dyn std::error::Error + Send + Sync>
        })?
    }
}
