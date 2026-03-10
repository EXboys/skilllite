//! 执行类命令：Run, Exec, Bash, Scan, Validate, Info, SecurityScan

use std::io::Read;

use anyhow::Context;
use skilllite_core::path_validation::validate_skill_path;
use skilllite_core::skill::metadata::parse_skill_metadata;

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Run {
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
        } = cmd
        {
            let run = || {
                if *resume || goal.is_some() {
                    #[cfg(feature = "agent")]
                    {
                        let g = goal.as_deref().unwrap_or("");
                        skilllite_agent::chat::run_agent_run(
                            None,
                            None,
                            None,
                            workspace.clone(),
                            skill_dirs.clone(),
                            soul.clone(),
                            g.to_string(),
                            *max_iterations,
                            true,
                            *max_failures,
                            *resume,
                        )
                    }
                    #[cfg(not(feature = "agent"))]
                    {
                        anyhow::bail!(
                            "Agent run mode requires the agent feature. Build with: cargo build --features agent"
                        )
                    }
                } else if let (Some(sd), Some(ij)) = (skill_dir, input_json) {
                    let input_json = if ij == "-" {
                        let mut s = String::new();
                        std::io::stdin().read_to_string(&mut s)?;
                        s
                    } else {
                        ij.clone()
                    };
                    let skill_path = validate_skill_path(sd)?;
                    let meta = parse_skill_metadata(&skill_path)?;
                    // 无入口时用大模型从 SKILL.md 推理入口，再执行（仅 agent feature 且已配 API）
                    let inferred_entry = if meta.entry_point.is_empty() {
                        #[cfg(feature = "agent")]
                        {
                            let config = skilllite_agent::types::AgentConfig::from_env();
                            if config.api_key.is_empty() {
                                None
                            } else {
                                let rt = tokio::runtime::Runtime::new()
                                    .context("tokio runtime for entry inference")?;
                                let llm = skilllite_agent::llm::LlmClient::new(
                                    &config.api_base,
                                    &config.api_key,
                                );
                                let adapter =
                                    skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };
                                rt.block_on(skilllite_agent::skills::infer_entry::infer_entry_point_from_skill_md(
                                    &skill_path,
                                    &adapter,
                                    &config.model,
                                ))
                                .ok()
                                .flatten()
                            }
                        }
                        #[cfg(not(feature = "agent"))]
                        {
                            None
                        }
                    } else {
                        None
                    };
                    let entry_override = inferred_entry.as_deref();
                    let sandbox_level =
                        skilllite_sandbox::runner::SandboxLevel::from_env_or_cli(*sandbox_level);
                    let limits = skilllite_sandbox::runner::ResourceLimits::from_env()
                        .with_cli_overrides(*max_memory, *timeout);
                    let result = skilllite_commands::execute::run_skill(
                        sd,
                        &input_json,
                        *allow_network,
                        cache_dir.as_ref(),
                        limits,
                        sandbox_level,
                        entry_override,
                    )?;
                    println!("{}", result);
                    Ok(())
                } else {
                    anyhow::bail!(
                        "Use either: skilllite run <SKILL_DIR> '<INPUT_JSON>'  OR  skilllite run --goal \"...\" [--soul SOUL.md]  OR  skilllite run --resume"
                    )
                }
            };
            Some(run())
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Exec {
            skill_dir,
            script_path,
            input_json,
            args,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
        } = cmd
        {
            let run = || {
                let input_json = if input_json == "-" {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s)?;
                    s
                } else {
                    input_json.clone()
                };
                let sandbox_level =
                    skilllite_sandbox::runner::SandboxLevel::from_env_or_cli(*sandbox_level);
                let limits = skilllite_sandbox::runner::ResourceLimits::from_env()
                    .with_cli_overrides(*max_memory, *timeout);
                let result = skilllite_commands::execute::exec_script(
                    skill_dir,
                    script_path,
                    &input_json,
                    args.as_ref(),
                    *allow_network,
                    cache_dir.as_ref(),
                    limits,
                    sandbox_level,
                )?;
                println!("{}", result);
                Ok(())
            };
            Some(run())
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Bash {
            skill_dir,
            command,
            cache_dir,
            timeout,
            cwd,
        } = cmd
        {
            let r = (|| {
                let result = skilllite_commands::execute::bash_command(
                    skill_dir,
                    command,
                    cache_dir.as_ref(),
                    timeout.unwrap_or(120),
                    cwd.as_ref(),
                )?;
                println!("{}", result);
                Ok::<(), anyhow::Error>(())
            })();
            Some(r)
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Scan {
            skill_dir,
            preview_lines,
        } = cmd
        {
            let r = (|| {
                let result = skilllite_commands::scan::scan_skill(skill_dir, *preview_lines)?;
                println!("{}", result);
                Ok::<(), anyhow::Error>(())
            })();
            Some(r)
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Validate { skill_dir } = cmd {
            let r = (|| {
                skilllite_commands::execute::validate_skill(skill_dir)?;
                println!("Skill validation passed!");
                Ok::<(), anyhow::Error>(())
            })();
            Some(r)
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Info { skill_dir } = cmd {
            Some(skilllite_commands::execute::show_skill_info(skill_dir))
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::SecurityScan {
            script_path,
            allow_network,
            allow_file_ops,
            allow_process_exec,
            json,
        } = cmd
        {
            Some(skilllite_commands::security::security_scan_script(
                script_path,
                *allow_network,
                *allow_file_ops,
                *allow_process_exec,
                *json,
            ))
        } else {
            None
        }
    });
}
