//! 命令分发：各领域命令的 register 逻辑，新增命令在此模块注册即可。

mod execute;
mod protocol;
mod skill;

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;

/// 注册所有命令处理器
pub fn register_all(reg: &mut CommandRegistry) {
    protocol::register(reg);
    execute::register(reg);
    skill::register(reg);
    register_ide(reg);
    register_env(reg);
    register_reindex(reg);
    register_security(reg);
    register_init(reg);
    #[cfg(feature = "agent")]
    {
        register_quickstart(reg);
        register_agent(reg);
    }
}

fn register_ide(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::InitCursor {
            project_dir,
            skills_dir,
            global,
            force,
        } = cmd
        {
            Some(
                skilllite_commands::ide::cmd_cursor(
                    project_dir.as_deref(),
                    skills_dir,
                    *global,
                    *force,
                )
                .map_err(Into::into),
            )
        } else if let Commands::InitOpencode {
            project_dir,
            skills_dir,
            force,
        } = cmd
        {
            Some(
                skilllite_commands::ide::cmd_opencode(project_dir.as_deref(), skills_dir, *force)
                    .map_err(Into::into),
            )
        } else {
            None
        }
    });
}

fn register_env(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::CleanEnv { dry_run, force } = cmd {
            Some(skilllite_commands::env::cmd_clean(*dry_run, *force).map_err(Into::into))
        } else {
            None
        }
    });
}

fn register_reindex(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Reindex {
            skills_dir,
            verbose,
            rebuild_manifest,
        } = cmd
        {
            Some(
                skilllite_commands::reindex::cmd_reindex(skills_dir, *verbose, *rebuild_manifest)
                    .map_err(Into::into),
            )
        } else {
            None
        }
    });
}

fn register_security(reg: &mut CommandRegistry) {
    #[cfg(feature = "audit")]
    reg.register(|cmd| {
        if let Commands::DependencyAudit { skill_dir, json } = cmd {
            Some(
                skilllite_commands::security::dependency_audit_skill(skill_dir, *json)
                    .map_err(Into::into),
            )
        } else {
            None
        }
    });
}

fn register_init(reg: &mut CommandRegistry) {
    #[cfg(feature = "agent")]
    reg.register(|cmd| {
        if let Commands::Init {
            skills_dir,
            skip_deps,
            skip_audit,
            strict,
            force,
            use_llm,
        } = cmd
        {
            Some(
                skilllite_commands::init::cmd_init(
                    skills_dir,
                    *skip_deps,
                    *skip_audit,
                    *strict,
                    *force,
                    *use_llm,
                )
                .map_err(Into::into),
            )
        } else {
            None
        }
    });
    #[cfg(not(feature = "agent"))]
    reg.register(|cmd| {
        if let Commands::Init {
            skills_dir,
            skip_deps,
            skip_audit,
            strict,
            force,
            ..
        } = cmd
        {
            Some(
                skilllite_commands::init::cmd_init(
                    skills_dir,
                    *skip_deps,
                    *skip_audit,
                    *strict,
                    *force,
                    false,
                )
                .map_err(Into::into),
            )
        } else {
            None
        }
    });
}

#[cfg(feature = "agent")]
fn register_quickstart(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Quickstart { skills_dir } = cmd {
            Some(skilllite_commands::quickstart::cmd_quickstart(skills_dir).map_err(Into::into))
        } else {
            None
        }
    });
}

#[cfg(feature = "agent")]
fn register_agent(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Chat {
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
        } = cmd
        {
            let mut config = skilllite_agent::types::AgentConfig::from_env();
            if let Some(base) = api_base.clone() {
                config.api_base = base;
            }
            if let Some(key) = api_key.clone() {
                config.api_key = key;
            }
            if let Some(m) = model.clone() {
                config.model = m;
            }
            if let Some(ws) = workspace.clone() {
                config.workspace = ws;
            }
            config.skill_dirs = skill_dir.clone();
            config.max_iterations = *max_iterations;
            config.system_prompt = system_prompt.clone();
            config.verbose = *verbose;
            config.soul_path = soul.clone();
            if *plan {
                config.enable_task_planning = true;
            } else if *no_plan {
                config.enable_task_planning = false;
            }
            config.enable_memory = !*no_memory;
            Some(
                skilllite_agent::chat::run_chat(config, session.clone(), message.clone())
                    .map_err(Into::into),
            )
        } else {
            None
        }
    });
    reg.register(|cmd| {
        if let Commands::ClearSession {
            session_key,
            workspace,
        } = cmd
        {
            Some(
                skilllite_agent::chat::run_clear_session(session_key, workspace)
                    .map_err(Into::into),
            )
        } else {
            None
        }
    });
    reg.register(|cmd| {
        if let Commands::Replay {
            dataset,
            api_base,
            api_key,
            model,
            workspace,
            skill_dir,
            max_iterations,
            max_failures,
            limit,
            json,
            verbose,
            read_only,
        } = cmd
        {
            let mut config = skilllite_agent::types::AgentConfig::from_env();
            if let Some(base) = api_base.clone() {
                config.api_base = base;
            }
            if let Some(key) = api_key.clone() {
                config.api_key = key;
            }
            if let Some(m) = model.clone() {
                config.model = m;
            }
            if let Some(ws) = workspace.clone() {
                config.workspace = ws;
            }
            config.skill_dirs = skill_dir.clone();
            config.max_iterations = *max_iterations;
            config.verbose = *verbose;
            config.read_only_tools = *read_only;
            if *read_only {
                config.context_append = Some(
                    "Replay is running in read-only evaluation mode. \
                     You must not modify files, write outputs, write memory, execute shell commands, \
                     start preview servers, delegate tasks, or execute skills. \
                     Only inspect the workspace and report findings."
                        .to_string(),
                );
            }
            config.enable_task_planning = true;
            config.enable_memory = true;
            config.max_consecutive_failures = match *max_failures {
                Some(0) => None,
                Some(n) => Some(n),
                None => Some(5),
            };
            Some(
                skilllite_commands::replay::cmd_replay(
                    config,
                    dataset.clone(),
                    *limit,
                    *json,
                )
                .map_err(Into::into),
            )
        } else {
            None
        }
    });
    reg.register(|cmd| {
        if let Commands::Evolution { action } = cmd {
            use crate::cli::EvolutionAction;
            let r = match action {
                EvolutionAction::Status => skilllite_commands::evolution::cmd_status(),
                EvolutionAction::Reset { force } => {
                    skilllite_commands::evolution::cmd_reset(*force)
                }
                EvolutionAction::Disable { rule_id } => {
                    skilllite_commands::evolution::cmd_disable(rule_id)
                }
                EvolutionAction::Explain { rule_id } => {
                    skilllite_commands::evolution::cmd_explain(rule_id)
                }
                EvolutionAction::Confirm { skill_name } => {
                    skilllite_commands::evolution::cmd_confirm(skill_name)
                }
                EvolutionAction::Reject { skill_name } => {
                    skilllite_commands::evolution::cmd_reject(skill_name)
                }
                EvolutionAction::Run { json } => skilllite_commands::evolution::cmd_run(*json),
                EvolutionAction::RepairSkills {
                    skills,
                    from_source,
                } => skilllite_commands::evolution::cmd_repair_skills(
                    if skills.is_empty() {
                        None
                    } else {
                        Some(skills.clone())
                    },
                    *from_source,
                ),
            };
            Some(r.map_err(Into::into))
        } else {
            None
        }
    });
}
