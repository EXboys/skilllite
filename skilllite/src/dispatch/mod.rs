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
            Some(skilllite_commands::ide::cmd_cursor(
                project_dir.as_deref(),
                skills_dir,
                *global,
                *force,
            ))
        } else if let Commands::InitOpencode {
            project_dir,
            skills_dir,
            force,
        } = cmd
        {
            Some(skilllite_commands::ide::cmd_opencode(
                project_dir.as_deref(),
                skills_dir,
                *force,
            ))
        } else {
            None
        }
    });
}

fn register_env(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::CleanEnv { dry_run, force } = cmd {
            Some(skilllite_commands::env::cmd_clean(*dry_run, *force))
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
            Some(skilllite_commands::reindex::cmd_reindex(
                skills_dir,
                *verbose,
                *rebuild_manifest,
            ))
        } else {
            None
        }
    });
}

fn register_security(reg: &mut CommandRegistry) {
    #[cfg(feature = "audit")]
    reg.register(|cmd| {
        if let Commands::DependencyAudit { skill_dir, json } = cmd {
            Some(skilllite_commands::security::dependency_audit_skill(
                skill_dir, *json,
            ))
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
            Some(skilllite_commands::init::cmd_init(
                skills_dir,
                *skip_deps,
                *skip_audit,
                *strict,
                *force,
                *use_llm,
            ))
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
            Some(skilllite_commands::init::cmd_init(
                skills_dir,
                *skip_deps,
                *skip_audit,
                *strict,
                *force,
                false,
            ))
        } else {
            None
        }
    });
}

#[cfg(feature = "agent")]
fn register_quickstart(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Quickstart { skills_dir } = cmd {
            Some(skilllite_commands::quickstart::cmd_quickstart(skills_dir))
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
                Some(skilllite_agent::chat::run_chat(
                    api_base.clone(),
                    api_key.clone(),
                    model.clone(),
                    workspace.clone(),
                    skill_dir.clone(),
                    session.clone(),
                    *max_iterations,
                    system_prompt.clone(),
                    *verbose,
                    message.clone(),
                    *plan,
                    *no_plan,
                    *no_memory,
                    soul.clone(),
                ))
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
                Some(skilllite_agent::chat::run_clear_session(
                    session_key, workspace,
                ))
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
                    EvolutionAction::Run { json } => {
                        skilllite_commands::evolution::cmd_run(*json)
                    }
                };
                Some(r)
            } else {
                None
            }
        });
}
