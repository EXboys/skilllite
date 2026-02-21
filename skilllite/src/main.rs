mod cli;
mod commands;
mod config;
mod env;
mod stdio_rpc;
mod mcp;
mod observability;
mod path_validation;
// mod protocol;
mod sandbox;
mod skill;

#[cfg(feature = "executor")]
mod executor;

#[cfg(feature = "agent")]
mod agent;

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    #[cfg(feature = "agent")]
    let is_chat = matches!(cli.command, Commands::Chat { .. });
    #[cfg(not(feature = "agent"))]
    let is_chat = false;
    observability::init_tracing(if is_chat { observability::TracingMode::Chat } else { observability::TracingMode::Default });

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                stdio_rpc::serve_stdio()?;
            }
        }
        Commands::Run {
            skill_dir,
            input_json,
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
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = commands::execute::run_skill(&skill_dir, &input_json, allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
            println!("{}", result);
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
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = commands::execute::exec_script(&skill_dir, &script_path, &input_json, args.as_ref(), allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
            println!("{}", result);
        }
        Commands::Bash {
            skill_dir,
            command,
            cache_dir,
            timeout,
            cwd,
        } => {
            let result = commands::execute::bash_command(&skill_dir, &command, cache_dir.as_ref(), timeout.unwrap_or(120), cwd.as_ref())?;
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
            commands::security::security_scan_script(&script_path, allow_network, allow_file_ops, allow_process_exec, json)?;
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
        } => {
            run_chat(
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
            )?;
        }

        // ─── Phase 3: CLI Migration Commands (flat) ─────────────────────

        Commands::Add { source, skills_dir, force, list } => {
            commands::skill::cmd_add(&source, &skills_dir, force, list)?;
        }
        Commands::Remove { skill_name, skills_dir, force } => {
            commands::skill::cmd_remove(&skill_name, &skills_dir, force)?;
        }
        Commands::List { skills_dir, json } => {
            commands::skill::cmd_list(&skills_dir, json)?;
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
        Commands::Show { skill_name, skills_dir, json } => {
            commands::skill::cmd_show(&skill_name, &skills_dir, json)?;
        }
        Commands::InitCursor { project_dir, skills_dir, global, force } => {
            commands::ide::cmd_cursor(project_dir.as_deref(), &skills_dir, global, force)?;
        }
        Commands::InitOpencode { project_dir, skills_dir, force } => {
            commands::ide::cmd_opencode(project_dir.as_deref(), &skills_dir, force)?;
        }
        #[cfg(feature = "audit")]
        Commands::DependencyAudit { skill_dir, json } => {
            commands::security::dependency_audit_skill(&skill_dir, json)?;
        }
        Commands::CleanEnv { dry_run, force } => {
            commands::env::cmd_clean(dry_run, force)?;
        }
        Commands::Reindex { skills_dir, verbose } => {
            commands::reindex::cmd_reindex(&skills_dir, verbose)?;
        }
        #[cfg(feature = "agent")]
        Commands::Quickstart { skills_dir } => {
            commands::quickstart::cmd_quickstart(&skills_dir)?;
        }
        #[cfg(feature = "agent")]
        Commands::Init { skills_dir, skip_deps, skip_audit, strict, force, use_llm } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, force, use_llm)?;
        }
        #[cfg(not(feature = "agent"))]
        Commands::Init { skills_dir, skip_deps, skip_audit, strict, force, .. } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, force, false)?;
        }
        #[cfg(feature = "agent")]
        Commands::AgentRpc => {
            agent::rpc::serve_agent_rpc()?;
        }
        Commands::Mcp { skills_dir } => {
            mcp::serve_mcp_stdio(&skills_dir)?;
        }
    }

    Ok(())
}

// ─── Agent Chat (feature = "agent") ─────────────────────────────────────────

#[cfg(feature = "agent")]
fn run_chat(
    api_base: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    workspace: Option<String>,
    skill_dirs: Vec<String>,
    session_key: String,
    max_iterations: usize,
    system_prompt: Option<String>,
    verbose: bool,
        single_message: Option<String>,
        plan: bool,
        no_plan: bool,
        no_memory: bool,
    ) -> Result<()> {
    use agent::types::*;
    use agent::{chat_session::ChatSession, skills};

    // Build config from CLI args + env
    let mut config = AgentConfig::from_env();
    if let Some(base) = api_base {
        config.api_base = base;
    }
    if let Some(key) = api_key {
        config.api_key = key;
    }
    if let Some(m) = model {
        config.model = m;
    }
    if let Some(ws) = workspace {
        config.workspace = ws;
    }
    config.max_iterations = max_iterations;
    config.system_prompt = system_prompt;
    config.verbose = verbose;

    // Set default output directory to ~/.skilllite/chat/output/ (matching Python SDK)
    // Only if SKILLLITE_OUTPUT_DIR is not already set by user
    // SAFETY: Called before tokio::runtime::Runtime::new() below, so no other
    // threads exist yet. All env var mutations happen in the single main thread.
    let paths = crate::config::PathsConfig::from_env();
    if paths.output_dir.is_none() {
        let chat_output = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output");
        let s = chat_output.to_string_lossy().to_string();
        unsafe { std::env::set_var("SKILLLITE_OUTPUT_DIR", &s) };
        // Ensure output directory exists
        let p = PathBuf::from(&s);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    } else if let Some(ref output_dir) = paths.output_dir {
        let p = PathBuf::from(output_dir);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }

    // Enable task planning: --plan > --no-plan > config (default true)
    if plan {
        config.enable_task_planning = true;
    } else if no_plan {
        config.enable_task_planning = false;
    }

    config.enable_memory = !no_memory;

    // Validate API key
    if config.api_key.is_empty() {
        anyhow::bail!(
            "API key required. Set OPENAI_API_KEY env var or use --api-key flag."
        );
    }

    // Auto-discover skill directories if none specified
    let (effective_skill_dirs, was_auto_discovered) = if skill_dirs.is_empty() {
        let ws = Path::new(&config.workspace);
        let mut auto_dirs = Vec::new();
        for name in &[".skills", "skills"] {
            let dir = ws.join(name);
            if dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join("SKILL.md").exists() {
                            auto_dirs.push(path.to_string_lossy().to_string());
                        }
                    }
                }
                if auto_dirs.is_empty() {
                    auto_dirs.push(dir.to_string_lossy().to_string());
                }
            }
        }
        let has_skills = !auto_dirs.is_empty();
        (auto_dirs, has_skills)
    } else {
        (skill_dirs, false)
    };

    // Load skills & print banner
    let loaded_skills = skills::load_skills(&effective_skill_dirs);
    if !loaded_skills.is_empty() {
        eprintln!("┌─ Skills ─────────────────────────────────────────────────");
        if was_auto_discovered {
            eprintln!("│  🔍 Auto-discovered {} skill(s)", loaded_skills.len());
        }
        let names: Vec<&str> = loaded_skills.iter().map(|s| s.name.as_str()).collect();
        let list = if names.len() <= 6 {
            names.join(", ")
        } else {
            format!("{} … +{} more", names[..5].join(", "), names.len() - 5)
        };
        eprintln!("│  📦 {}", list);
        eprintln!("└───────────────────────────────────────────────────────────");
    }

    // Build tokio runtime
    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    if let Some(msg) = single_message {
        // Single-shot mode
        rt.block_on(async {
            let mut session = ChatSession::new(config, &session_key, loaded_skills);
            let mut sink = TerminalEventSink::new(verbose);
            let response = session.run_turn(&msg, &mut sink).await?;
            println!("\n{}", response);
            Ok(())
        })
    } else {
        // Interactive REPL mode
        rt.block_on(async {
            run_interactive_chat(config, &session_key, loaded_skills, verbose).await
        })
    }
}

/// Format agent/API errors for user-friendly display in chat UI.
#[cfg(feature = "agent")]
fn format_chat_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
    // Try to extract error.message from LLM API JSON: "LLM API error (403): {...}"
    if let Some(json_start) = s.find('{') {
        let json_part = &s[json_start..];
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_part) {
            if let Some(msg) = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
            {
                let status = s
                    .strip_prefix("LLM API error (")
                    .and_then(|rest| rest.split(')').next())
                    .unwrap_or("API");
                return format!("{} 错误: {}", status, msg);
            }
        }
    }
    // Fallback: truncate long errors (e.g. full JSON dump)
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s
    }
}

#[cfg(feature = "agent")]
async fn run_interactive_chat(
    config: agent::types::AgentConfig,
    session_key: &str,
    skills: Vec<agent::skills::LoadedSkill>,
    verbose: bool,
) -> Result<()> {
    use agent::types::*;
    use agent::chat_session::ChatSession;

    eprintln!("┌────────────────────────────────────────────────────────────");
    eprintln!("│  🤖 SkillBox Chat  ·  model: {}", config.model);
    eprintln!("│  /exit 退出  ·  /clear 清空  ·  /compact 压缩历史");
    eprintln!("└────────────────────────────────────────────────────────────\n");

    let mut session = ChatSession::new(config, session_key, skills);
    let mut sink = TerminalEventSink::new(verbose);

    // Setup rustyline for readline-like input
    let mut rl = rustyline::DefaultEditor::new()
        .map_err(|e| anyhow::anyhow!("Failed to create line editor: {}", e))?;

    loop {
        let readline = rl.readline("You> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(input);

                // Handle slash commands
                match input {
                    "/exit" | "/quit" | "/q" => {
                        eprintln!("👋 Bye!");
                        break;
                    }
                    "/clear" => {
                        session.clear().await?;
                        eprintln!("🗑️  Session cleared.");
                        continue;
                    }
                    "/compact" => {
                        eprintln!("📦 Compacting history...");
                        match session.force_compact().await {
                            Ok(true) => eprintln!("✅ History compacted."),
                            Ok(false) => eprintln!("ℹ️  Not enough messages to compact."),
                            Err(e) => eprintln!("❌ Compaction failed: {}", format_chat_error(&e)),
                        }
                        continue;
                    }
                    _ => {}
                }

                // Run the turn
                eprintln!();
                match session.run_turn(input, &mut sink).await {
                    Ok(_) => {
                        eprintln!();
                    }
                    Err(e) => {
                        let msg = format_chat_error(&e);
                        eprintln!("❌ {}", msg);
                        eprintln!();
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("\n^C");
                eprintln!("👋 Bye!");
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                eprintln!("👋 Bye!");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
