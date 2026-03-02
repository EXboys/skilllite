//! 协议类命令：Serve, AgentRpc, Swarm, Mcp

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;
use crate::protocol::ProtocolHandler;

#[cfg(all(feature = "agent", feature = "swarm"))]
use crate::swarm_executor;

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Serve { stdio } = cmd {
            if *stdio {
                Some(
                    crate::protocol::StdioRpcHandler
                        .serve(crate::protocol::ProtocolParams::Stdio),
                )
            } else {
                Some(Ok(()))
            }
        } else {
            None
        }
    });

    #[cfg(feature = "agent")]
    reg.register(|cmd| {
        if matches!(cmd, Commands::AgentRpc) {
            Some(
                crate::protocol::AgentRpcHandler
                    .serve(crate::protocol::ProtocolParams::AgentRpc),
            )
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Swarm { listen, skills_dir } = cmd {
            let capability_tags = crate::aggregate_capability_tags(skills_dir.as_deref());
            #[cfg(feature = "swarm")]
            {
                #[cfg(feature = "agent")]
                let executor: Option<std::sync::Arc<dyn skilllite_swarm::TaskExecutor>> =
                    Some(std::sync::Arc::new(swarm_executor::AgentTaskExecutor::new(
                        skills_dir.clone(),
                    )));
                #[cfg(not(feature = "agent"))]
                let executor: Option<std::sync::Arc<dyn skilllite_swarm::TaskExecutor>> = None;
                Some(
                    crate::protocol::SwarmHandler.serve(crate::protocol::ProtocolParams::P2p {
                        listen_addr: listen.clone(),
                        capability_tags,
                        skills_dir: skills_dir.clone(),
                        executor,
                    }),
                )
            }
            #[cfg(not(feature = "swarm"))]
            {
                Some(
                    crate::protocol::SwarmHandler.serve(crate::protocol::ProtocolParams::P2p {
                        listen_addr: listen.clone(),
                        capability_tags,
                        skills_dir: skills_dir.clone(),
                    }),
                )
            }
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Mcp { skills_dir } = cmd {
            Some(
                crate::protocol::McpHandler.serve(crate::protocol::ProtocolParams::Mcp {
                    skills_dir: skills_dir.clone(),
                }),
            )
        } else {
            None
        }
    });
}
