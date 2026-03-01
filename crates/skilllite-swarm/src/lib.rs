//! SkillLite P2P Swarm — mDNS discovery, peer mesh, task routing.
//!
//! This crate implements the swarm daemon for `skilllite swarm --listen <ADDR>`:
//! - **Discovery**: mDNS service registration and browsing for peer nodes
//! - **Routing**: Match required_capabilities with local/neighbor capabilities
//! - **HTTP /task**: Receive NodeTask, execute locally or forward to peer

mod discovery;
mod handler;
mod routing;

pub use discovery::{Discovery, PeerInfo};
pub use handler::serve_swarm;
pub use routing::{capabilities_match, route_task, RouteTarget, TaskExecutor};
