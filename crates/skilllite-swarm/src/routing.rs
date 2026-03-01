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
    /// Forward to peer(s). First is primary; rest are fallbacks when primary fails.
    Forward(Vec<PeerInfo>),
    /// No matching node found — cannot route.
    NoMatch,
}

/// Extract port from "host:port" addr.
fn port_from_addr(addr: &str) -> Option<u16> {
    addr.rsplit_once(':').and_then(|(_, p)| p.parse().ok())
}

/// Collect all peers that match required capabilities (for fallback retry).
/// Dedupe by port (same node may appear as 127.0.0.1:PORT and LAN_IP:PORT via mDNS).
/// Prefer 127.0.0.1 over LAN IP for same port — more reliable for same-machine forwarding.
fn matching_peers(required: &[String], peers: &[PeerInfo]) -> Vec<PeerInfo> {
    if required.is_empty() {
        return vec![];
    }
    let matching: Vec<_> = peers
        .iter()
        .filter(|p| capabilities_match(required, &p.capabilities))
        .cloned()
        .collect();
    // Dedupe by port: same node can appear as 127.0.0.1:7701 and 10.x:7701. Prefer loopback.
    let mut by_port: std::collections::HashMap<u16, PeerInfo> = std::collections::HashMap::new();
    for p in matching {
        if let Some(port) = port_from_addr(&p.addr) {
            let entry = by_port.entry(port).or_insert_with(|| p.clone());
            // Prefer 127.0.0.1 over LAN IP for same port (same-machine forwarding)
            if p.addr.starts_with("127.") && !entry.addr.starts_with("127.") {
                *entry = p;
            }
        }
    }
    let mut matching: Vec<_> = by_port.into_values().collect();
    // Sort: prefer 127.0.0.1 first, then by addr for stable order
    matching.sort_by(|a, b| {
        let a_loopback = a.addr.starts_with("127.");
        let b_loopback = b.addr.starts_with("127.");
        match (a_loopback, b_loopback) {
            (true, false) => std::cmp::Ordering::Less,   // loopback first
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.addr.cmp(&b.addr),
        }
    });
    matching
}

/// Decide routing for a NodeTask given local capabilities and discovered peers.
///
/// **Skill sharing**: When `required_capabilities` is empty, prefer forwarding to a peer
/// that has capabilities over executing locally with none. This enables "蜂群共享技能" —
/// a node without skills forwards to peers that have them.
pub fn route_task(
    task: &NodeTask,
    local_capabilities: &[String],
    peers: &[PeerInfo],
) -> RouteTarget {
    let required = &task.context.required_capabilities;

    // Explicit required: match local or peer
    if !required.is_empty() {
        if capabilities_match(required, local_capabilities) {
            return RouteTarget::Local;
        }
        let matching = matching_peers(required, peers);
        if !matching.is_empty() {
            return RouteTarget::Forward(matching);
        }
        return RouteTarget::NoMatch;
    }

    // required is empty — "skill sharing" mode: prefer peer with capabilities over local without
    let local_has_caps = !local_capabilities.is_empty();
    let peers_with_caps: Vec<PeerInfo> = peers
        .iter()
        .filter(|p| !p.capabilities.is_empty())
        .cloned()
        .collect();

    if !local_has_caps && !peers_with_caps.is_empty() {
        // Prefer peer with most capabilities; dedupe by addr (same port = same node, avoid stale mDNS)
        let mut sorted: Vec<_> = peers_with_caps;
        sorted.sort_by(|a, b| b.capabilities.len().cmp(&a.capabilities.len()));
        let mut seen_addr = std::collections::HashSet::new();
        let deduped: Vec<_> = sorted
            .into_iter()
            .filter(|p| seen_addr.insert(p.addr.clone()))
            .collect();
        if !deduped.is_empty() {
            return RouteTarget::Forward(deduped);
        }
    }

    // Local has caps, or no peer has caps — execute locally
    RouteTarget::Local
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

    #[test]
    fn test_route_skill_sharing_empty_required_local_no_caps_peer_has_caps() {
        let task = NodeTask {
            id: "t1".into(),
            description: "1+1=?".into(),
            context: skilllite_core::protocol::NodeContext {
                workspace: ".".into(),
                session_key: "test".into(),
                required_capabilities: vec![],
            },
            tool_hint: None,
        };
        let local: Vec<String> = vec![];
        let peers = vec![PeerInfo {
            instance_name: "peer1".into(),
            addr: "127.0.0.1:7701".into(),
            capabilities: vec!["calc".into()],
        }];
        let target = route_task(&task, &local, &peers);
        assert!(matches!(target, RouteTarget::Forward(ref v) if v.len() == 1 && v[0].instance_name == "peer1"));
    }

    #[test]
    fn test_matching_peers_dedup_by_port_prefer_loopback() {
        // Same node can appear as 127.0.0.1:7701 and 10.55.157.245:7701 via mDNS.
        // Should dedupe by port and prefer 127.0.0.1 for same-machine forwarding.
        let task = NodeTask {
            id: "t1".into(),
            description: "1+1=?".into(),
            context: skilllite_core::protocol::NodeContext {
                workspace: ".".into(),
                session_key: "test".into(),
                required_capabilities: vec!["calc".into()],
            },
            tool_hint: None,
        };
        let local: Vec<String> = vec![];
        let peers = vec![
            PeerInfo {
                instance_name: "peer-lan".into(),
                addr: "10.55.157.245:7701".into(),
                capabilities: vec!["calc".into()],
            },
            PeerInfo {
                instance_name: "peer-loopback".into(),
                addr: "127.0.0.1:7701".into(),
                capabilities: vec!["calc".into(), "math".into()],
            },
        ];
        let target = route_task(&task, &local, &peers);
        let RouteTarget::Forward(v) = target else { panic!("expected Forward") };
        assert_eq!(v.len(), 1, "should dedupe to one peer by port");
        assert!(v[0].addr.starts_with("127."), "should prefer 127.0.0.1 over LAN IP");
    }

    #[test]
    fn test_route_skill_sharing_empty_required_local_has_caps() {
        let task = NodeTask {
            id: "t1".into(),
            description: "1+1=?".into(),
            context: skilllite_core::protocol::NodeContext {
                workspace: ".".into(),
                session_key: "test".into(),
                required_capabilities: vec![],
            },
            tool_hint: None,
        };
        let local = vec!["calc".into()];
        let peers = vec![PeerInfo {
            instance_name: "peer1".into(),
            addr: "127.0.0.1:7701".into(),
            capabilities: vec!["calc".into(), "web".into()],
        }];
        let target = route_task(&task, &local, &peers);
        assert!(matches!(target, RouteTarget::Local));
    }
}
