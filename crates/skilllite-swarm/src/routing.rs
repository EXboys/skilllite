//! Task routing — match required_capabilities with local/neighbor capabilities.
//!
//! Phase 3: When a NodeTask is received, we match its `required_capabilities`
//! against local capability_tags and discovered peers. Local match → execute;
//! otherwise → forward to matching peer or broadcast "who can do".

use skilllite_core::protocol::{NodeTask, NodeResult};

use crate::discovery::PeerInfo;

/// Check if `available` capabilities satisfy all `required` capabilities.
/// Empty required = no constraint, always matches.
pub fn capabilities_match(required: &[String], available: &[String]) -> bool {
    if required.is_empty() {
        return true;
    }
    let avail: std::collections::HashSet<_> = available.iter().map(|s| s.as_str()).collect();
    required.iter().all(|r| avail.contains(r.as_str()))
}

/// Routing decision: where should the task go?
#[derive(Debug, Clone)]
pub enum RouteTarget {
    /// Execute locally (this node has matching capabilities).
    Local,
    /// Forward to this peer.
    Forward(PeerInfo),
    /// No matching node found — cannot route.
    NoMatch,
}

/// Decide routing for a NodeTask given local capabilities and discovered peers.
pub fn route_task(
    task: &NodeTask,
    local_capabilities: &[String],
    peers: &[PeerInfo],
) -> RouteTarget {
    let required = &task.context.required_capabilities;

    if capabilities_match(required, local_capabilities) {
        return RouteTarget::Local;
    }

    for peer in peers {
        if capabilities_match(required, &peer.capabilities) {
            return RouteTarget::Forward(peer.clone());
        }
    }

    RouteTarget::NoMatch
}

/// Executor trait: called when routing decides to execute locally.
/// Implemented by the skilllite binary (agent integration).
pub trait TaskExecutor: Send + Sync + std::fmt::Debug {
    /// Execute the task locally and return the result.
    fn execute(&self, task: NodeTask) -> Result<NodeResult, Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_match_empty_required() {
        assert!(capabilities_match(&[], &["python".into()]));
    }

    #[test]
    fn test_capabilities_match_satisfied() {
        assert!(capabilities_match(&["python".into()], &["python".into(), "web".into()]));
    }

    #[test]
    fn test_capabilities_match_missing() {
        assert!(!capabilities_match(&["python".into(), "ml".into()], &["python".into()]));
    }
}
