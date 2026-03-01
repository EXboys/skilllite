//! SkillLite CLI library — shared by skilllite and skilllite-sandbox binaries.

mod cli;
mod commands;
mod mcp;
mod protocol;
mod stdio_rpc;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use protocol::ProtocolHandler;
use std::collections::HashSet;
use std::io::Read;

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
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    #[cfg(feature = "agent")]
    let is_chat = matches!(cli.command, Commands::Chat { .. });
    #[cfg(not(feature = "agent"))]
    let is_chat = false;
    skilllite_core::observability::init_tracing(if is_chat {
        skilllite_core::observability::TracingMode::Chat
    } else {
        skilllite_core::observability::TracingMode::Default
    });

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                protocol::StdioRpcHandler
                    .serve(protocol::ProtocolParams::Stdio)?;
            }
        }
        Commands::Run {
            skill_dir,
            input_json,
            soul,
            goal,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
            workspace,
            skill_dirs,
            max_iterations,
            max_failures,
            resume,
        } => {
            if resume || goal.is_some() {
                #[cfg(feature = "agent")]
                {
                    let g = goal.unwrap_or_else(|| "".to_string());
                    skilllite_agent::chat::run_agent_run(
                        None,
                        None,
                        None,
                        workspace,
                        skill_dirs,
                        soul,
                        g,
                        max_iterations,
                        true,
                        max_failures,
                        resume,
                    )?;
                }
                #[cfg(not(feature = "agent"))]
                {
                    anyhow::bail!(
                        "Agent run mode requires the agent feature. Build with: cargo build --features agent"
                    );
                }
            } else if let (Some(sd), Some(ij)) = (skill_dir, input_json) {
                let input_json = if ij == "-" {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s)?;
                    s
                } else {
                    ij
                };
                let sandbox_level =
                    skilllite_sandbox::runner::SandboxLevel::from_env_or_cli(sandbox_level);
                let limits = skilllite_sandbox::runner::ResourceLimits::from_env()
                    .with_cli_overrides(max_memory, timeout);
                let result = commands::execute::run_skill(
                    &sd,
                    &input_json,
                    allow_network,
                    cache_dir.as_ref(),
                    limits,
                    sandbox_level,
                )?;
                println!("{}", result);
            } else {
                anyhow::bail!(
                    "Use either: skilllite run <SKILL_DIR> '<INPUT_JSON>'  OR  skilllite run --goal \"...\" [--soul SOUL.md]  OR  skilllite run --resume"
                );
            }
        }
        Commands::Exec {
            skill_dir,
            script_path,
            input_json,
            args,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
        } => {
            let input_json = if input_json == "-" {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s)?;
                s
            } else {
                input_json
            };
            let sandbox_level =
                skilllite_sandbox::runner::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = skilllite_sandbox::runner::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = commands::execute::exec_script(
                &skill_dir,
                &script_path,
                &input_json,
                args.as_ref(),
                allow_network,
                cache_dir.as_ref(),
                limits,
                sandbox_level,
            )?;
            println!("{}", result);
        }
        Commands::Bash {
            skill_dir,
            command,
            cache_dir,
            timeout,
            cwd,
        } => {
            let result = commands::execute::bash_command(
                &skill_dir,
                &command,
                cache_dir.as_ref(),
                timeout.unwrap_or(120),
                cwd.as_ref(),
            )?;
            println!("{}", result);
        }
        Commands::Scan {
            skill_dir,
            preview_lines,
        } => {
            let result = commands::scan::scan_skill(&skill_dir, preview_lines)?;
            println!("{}", result);
        }
        Commands::Validate { skill_dir } => {
            commands::execute::validate_skill(&skill_dir)?;
            println!("Skill validation passed!");
        }
        Commands::Info { skill_dir } => {
            commands::execute::show_skill_info(&skill_dir)?;
        }
        Commands::SecurityScan {
            script_path,
            allow_network,
            allow_file_ops,
            allow_process_exec,
            json,
        } => {
            commands::security::security_scan_script(
                &script_path,
                allow_network,
                allow_file_ops,
                allow_process_exec,
                json,
            )?;
        }
        #[cfg(feature = "agent")]
        Commands::Chat {
            api_base,
            api_key,
            model,
            workspace,
            skill_dir,
            session,
            max_iterations,
            system_prompt,
            verbose,
            message,
            plan,
            no_plan,
            no_memory,
            soul,
        } => {
            skilllite_agent::chat::run_chat(
                api_base,
                api_key,
                model,
                workspace,
                skill_dir,
                session,
                max_iterations,
                system_prompt,
                verbose,
                message,
                plan,
                no_plan,
                no_memory,
                soul,
            )?;
        }

        // ─── Phase 3: CLI Migration Commands (flat) ─────────────────────

        Commands::Add {
            source,
            skills_dir,
            force,
            list,
            scan_offline,
        } => {
            commands::skill::cmd_add(&source, &skills_dir, force, list, scan_offline)?;
        }
        Commands::Remove {
            skill_name,
            skills_dir,
            force,
        } => {
            commands::skill::cmd_remove(&skill_name, &skills_dir, force)?;
        }
        Commands::List { skills_dir, json, scan } => {
            commands::skill::cmd_list(&skills_dir, json, scan)?;
        }
        #[cfg(feature = "agent")]
        Commands::ListTools { skills_dir, format } => {
            let params = serde_json::json!({
                "skills_dir": skills_dir,
                "format": format
            });
            let result = stdio_rpc::handle_list_tools(&params)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Commands::Show {
            skill_name,
            skills_dir,
            json,
        } => {
            commands::skill::cmd_show(&skill_name, &skills_dir, json)?;
        }
        Commands::Verify {
            target,
            skills_dir,
            json,
            strict,
        } => {
            commands::skill::cmd_verify(&target, &skills_dir, json, strict)?;
        }
        Commands::InitCursor {
            project_dir,
            skills_dir,
            global,
            force,
        } => {
            commands::ide::cmd_cursor(project_dir.as_deref(), &skills_dir, global, force)?;
        }
        Commands::InitOpencode {
            project_dir,
            skills_dir,
            force,
        } => {
            commands::ide::cmd_opencode(project_dir.as_deref(), &skills_dir, force)?;
        }
        #[cfg(feature = "audit")]
        Commands::DependencyAudit { skill_dir, json } => {
            commands::security::dependency_audit_skill(&skill_dir, json)?;
        }
        Commands::CleanEnv { dry_run, force } => {
            commands::env::cmd_clean(dry_run, force)?;
        }
        Commands::Reindex {
            skills_dir,
            verbose,
            rebuild_manifest,
        } => {
            commands::reindex::cmd_reindex(&skills_dir, verbose, rebuild_manifest)?;
        }
        #[cfg(feature = "agent")]
        Commands::Quickstart { skills_dir } => {
            commands::quickstart::cmd_quickstart(&skills_dir)?;
        }
        #[cfg(feature = "agent")]
        Commands::Init {
            skills_dir,
            skip_deps,
            skip_audit,
            strict,
            force,
            use_llm,
        } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, force, use_llm)?;
        }
        #[cfg(not(feature = "agent"))]
        Commands::Init {
            skills_dir,
            skip_deps,
            skip_audit,
            strict,
            force,
            ..
        } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, force, false)?;
        }
        #[cfg(feature = "agent")]
        Commands::ClearSession {
            session_key,
            workspace,
        } => {
            skilllite_agent::chat::run_clear_session(&session_key, &workspace)?;
        }
        #[cfg(feature = "agent")]
        Commands::AgentRpc => {
            protocol::AgentRpcHandler
                .serve(protocol::ProtocolParams::AgentRpc)?;
        }
        Commands::Swarm { listen, skills_dir } => {
            let capability_tags = aggregate_capability_tags(skills_dir.as_deref());
            protocol::SwarmHandler.serve(protocol::ProtocolParams::P2p {
                listen_addr: listen,
                capability_tags,
            })?;
        }
        Commands::Mcp { skills_dir } => {
            protocol::McpHandler
                .serve(protocol::ProtocolParams::Mcp {
                    skills_dir: skills_dir.clone(),
                })?;
        }
        #[cfg(feature = "agent")]
        Commands::Evolution { action } => {
            use cli::EvolutionAction;
            match action {
                EvolutionAction::Status => commands::evolution::cmd_status()?,
                EvolutionAction::Reset { force } => commands::evolution::cmd_reset(force)?,
                EvolutionAction::Disable { rule_id } => commands::evolution::cmd_disable(&rule_id)?,
                EvolutionAction::Explain { rule_id } => commands::evolution::cmd_explain(&rule_id)?,
                EvolutionAction::Confirm { skill_name } => commands::evolution::cmd_confirm(&skill_name)?,
                EvolutionAction::Reject { skill_name } => commands::evolution::cmd_reject(&skill_name)?,
                EvolutionAction::Run { json } => commands::evolution::cmd_run(json)?,
            }
        }
    }

    Ok(())
}
