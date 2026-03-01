//! SkillLite P2P Swarm — mDNS discovery, peer mesh, task routing.
//!
//! This crate implements the swarm daemon for `skilllite swarm --listen <ADDR>`:
//! - **Discovery**: mDNS service registration and browsing for peer nodes
//! - **SwarmHandler**: Full daemon loop (register, browse, block until shutdown)

mod discovery;
mod handler;

pub use discovery::{Discovery, PeerInfo};
pub use handler::serve_swarm;
