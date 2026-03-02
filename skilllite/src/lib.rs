//! SkillLite CLI library — shared by skilllite and skilllite-sandbox binaries.

mod cli;
mod command_registry;
mod dispatch;
mod mcp;
mod protocol;
mod stdio_rpc;
#[cfg(all(feature = "agent", feature = "swarm"))]
mod swarm_executor;

use clap::Parser;
use cli::Cli;
use std::collections::HashSet;

/// Aggregate capability tags from skills. When agent feature is on, loads skills
/// from the given dirs (or [".skills", "skills"] if None) and collects capabilities.
#[cfg(feature = "agent")]
fn aggregate_capability_tags(skills_dir: Option<&[String]>) -> Vec<String> {
    let dirs: Vec<String> = skills_dir
        .map(|s| s.to_vec())
        .unwrap_or_else(|| vec![".skills".into(), "skills".into()]);
    let loaded = skilllite_agent::skills::load_skills(&dirs);
    let mut caps = HashSet::new();
    for skill in &loaded {
        for c in &skill.metadata.capabilities {
            caps.insert(c.clone());
        }
    }
    let mut v: Vec<_> = caps.into_iter().collect();
    v.sort();
    v
}

#[cfg(not(feature = "agent"))]
fn aggregate_capability_tags(_skills_dir: Option<&[String]>) -> Vec<String> {
    vec![]
}

/// Run the CLI — parses args and dispatches to command handlers.
/// Used by both `skilllite` (full) and `skilllite-sandbox` (minimal) binaries.
pub fn run_cli() -> anyhow::Result<()> {
    let cli = Cli::parse();
    #[cfg(feature = "agent")]
    let is_chat = matches!(cli.command, cli::Commands::Chat { .. });
    #[cfg(not(feature = "agent"))]
    let is_chat = false;
    skilllite_core::observability::init_tracing(if is_chat {
        skilllite_core::observability::TracingMode::Chat
    } else {
        skilllite_core::observability::TracingMode::Default
    });

    let mut reg = command_registry::CommandRegistry::new();
    dispatch::register_all(&mut reg);
    reg.dispatch(&cli.command)
}
