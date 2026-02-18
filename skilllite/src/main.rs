mod cli;
mod commands;
mod config;
mod env;
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
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use clap::Parser;
use cli::{Cli, Commands};
use serde_json::{json, Value};

fn main() -> Result<()> {
    observability::init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { stdio } => {
            if stdio {
                serve_stdio()?;
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
            let result = handle_list_tools(&params)?;
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
        Commands::Init { skills_dir, skip_deps, skip_audit, strict, use_llm } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, use_llm)?;
        }
        #[cfg(not(feature = "agent"))]
        Commands::Init { skills_dir, skip_deps, skip_audit, strict, .. } => {
            commands::init::cmd_init(&skills_dir, skip_deps, skip_audit, strict, false)?;
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

/// IPC daemon: read JSON-RPC requests from stdin (one per line), write responses to stdout.
/// Uses thread pool for concurrent request handling (run/exec/bash run in parallel).
/// Request: {"jsonrpc":"2.0","id":1,"method":"run"|"exec","params":{...}}
/// Response: {"jsonrpc":"2.0","id":1,"result":{...}} or {"jsonrpc":"2.0","id":1,"error":{...}}
fn serve_stdio() -> Result<()> {
    // Suppress info logs in daemon mode (benchmark, etc.)
    // SAFETY: Called at the start of serve_stdio before any multi-threading.
    unsafe {
        std::env::set_var("SKILLBOX_AUTO_APPROVE", "1");
        std::env::set_var("SKILLLITE_QUIET", "1");
    }

    let (tx, rx) = mpsc::channel::<(Value, std::result::Result<Value, String>)>();

    // Writer thread: receives results and writes to stdout (stdout is not Sync)
    let writer_handle = thread::spawn(move || -> Result<()> {
        let mut stdout = io::stdout();
        for (id, result) in rx {
            match result {
                Ok(res) => {
                    let resp = json!({"jsonrpc": "2.0", "id": id, "result": res});
                    writeln!(stdout, "{}", resp)?;
                }
                Err(msg) => {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32603, "message": msg}
                    });
                    writeln!(stdout, "{}", err_resp)?;
                }
            }
            stdout.flush()?;
        }
        Ok(())
    });

    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let mut pending = 0usize;

    for line in reader.lines() {
        let line = match line.context("Failed to read stdin") {
            Ok(l) => l,
            Err(e) => {
                let _ = tx.send((Value::Null, Err(e.to_string())));
                continue;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send((Value::Null, Err(format!("Parse error: {}", e))));
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        pending += 1;
        let tx = tx.clone();
        let done_tx = done_tx.clone();
        rayon::spawn(move || {
            let result = dispatch_ipc_request(&method, &params);
            let _ = tx.send((id, result.map_err(|e| e.to_string())));
            let _ = done_tx.send(());
        });
    }

    for _ in 0..pending {
        let _ = done_rx.recv();
    }
    drop(tx);
    writer_handle.join().map_err(|_| anyhow::anyhow!("Writer thread panicked"))??;

    Ok(())
}

/// Dispatch IPC request to the appropriate handler. Runs in thread pool.
fn dispatch_ipc_request(method: &str, params: &Value) -> Result<Value> {
    match method {
        "run" => handle_run(params),
        "exec" => handle_exec(params),
        "bash" => handle_bash(params),
        #[cfg(feature = "executor")]
        "session_create" => executor::rpc::handle_session_create(params),
        #[cfg(feature = "executor")]
        "session_get" => executor::rpc::handle_session_get(params),
        #[cfg(feature = "executor")]
        "session_update" => executor::rpc::handle_session_update(params),
        #[cfg(feature = "executor")]
        "transcript_append" => executor::rpc::handle_transcript_append(params),
        #[cfg(feature = "executor")]
        "transcript_read" => executor::rpc::handle_transcript_read(params),
        #[cfg(feature = "executor")]
        "transcript_ensure" => executor::rpc::handle_transcript_ensure(params),
        #[cfg(feature = "executor")]
        "memory_write" => executor::rpc::handle_memory_write(params),
        #[cfg(feature = "executor")]
        "memory_search" => executor::rpc::handle_memory_search(params),
        #[cfg(feature = "executor")]
        "token_count" => executor::rpc::handle_token_count(params),
        #[cfg(feature = "executor")]
        "plan_textify" => executor::rpc::handle_plan_textify(params),
        #[cfg(feature = "executor")]
        "plan_write" => executor::rpc::handle_plan_write(params),
        #[cfg(feature = "executor")]
        "plan_read" => executor::rpc::handle_plan_read(params),
        #[cfg(feature = "agent")]
        "build_skills_context" => handle_build_skills_context(params),
        #[cfg(feature = "agent")]
        "list_tools" => handle_list_tools(params),
        _ => anyhow::bail!("Method not found: {}", method),
    }
}

fn handle_run(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = sandbox::executor::ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = commands::execute::run_skill(skill_dir, input_json, allow_network, cache_dir_ref, limits, sandbox_level)?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_exec(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let script_path = p.get("script_path").and_then(|v| v.as_str()).context("script_path required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let args = p.get("args").and_then(|v| v.as_str()).map(|s| s.to_string());
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = sandbox::executor::ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = commands::execute::exec_script(
        skill_dir,
        script_path,
        input_json,
        args.as_ref(),
        allow_network,
        cache_dir_ref,
        limits,
        sandbox_level,
    )?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_bash(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let command = p.get("command").and_then(|v| v.as_str()).context("command required")?;
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout = p.get("timeout").and_then(|v| v.as_u64()).unwrap_or(120);
    let cwd = p.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());

    let output = commands::execute::bash_command(skill_dir, command, cache_dir.as_ref(), timeout, cwd.as_ref())?;
    Ok(serde_json::from_str(&output).unwrap_or_else(|_| json!({
        "output": output,
        "exit_code": 0
    })))
}

#[cfg(feature = "agent")]
fn handle_build_skills_context(params: &Value) -> Result<Value> {
    use agent::prompt::{build_skills_context, PromptMode};
    use agent::skills;

    let p = params.as_object().context("params must be object")?;
    let skills_dir = p.get("skills_dir").and_then(|v| v.as_str()).context("skills_dir required")?;
    let mode_str = p.get("mode").and_then(|v| v.as_str()).unwrap_or("progressive");
    let skills_filter: Option<Vec<String>> = p
        .get("skills")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let skills_path = path_validation::validate_path_under_root(skills_dir, "skills_dir")?;
    let skills_path_str = skills_path.to_string_lossy().to_string();

    let loaded = skills::load_skills(&[skills_path_str]);
    let loaded: Vec<_> = if let Some(ref filter) = skills_filter {
        loaded
            .into_iter()
            .filter(|s| filter.contains(&s.name))
            .collect()
    } else {
        loaded
    };

    let mode = match mode_str {
        "summary" => PromptMode::Summary,
        "standard" => PromptMode::Standard,
        "full" => PromptMode::Full,
        _ => PromptMode::Progressive,
    };

    let context = build_skills_context(&loaded, mode);
    Ok(json!({ "context": context }))
}

#[cfg(feature = "agent")]
fn handle_list_tools(params: &Value) -> Result<Value> {
    use agent::skills;

    let p = params.as_object().context("params must be object")?;
    let skills_dir = p.get("skills_dir").and_then(|v| v.as_str()).context("skills_dir required")?;
    let skills_filter: Option<Vec<String>> = p
        .get("skills")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
    let format_str = p.get("format").and_then(|v| v.as_str()).unwrap_or("openai");

    let skills_path = path_validation::validate_path_under_root(skills_dir, "skills_dir")?;
    let skills_path_str = skills_path.to_string_lossy().to_string();

    let loaded = skills::load_skills(&[skills_path_str]);
    let loaded: Vec<_> = if let Some(ref filter) = skills_filter {
        loaded
            .into_iter()
            .filter(|s| filter.contains(&s.name))
            .collect()
    } else {
        loaded
    };

    let mut tools: Vec<Value> = Vec::new();
    let mut tool_meta: serde_json::Map<String, Value> = serde_json::Map::new();
    for skill in &loaded {
        let skill_dir_str = skill.skill_dir.to_string_lossy().to_string();
        for td in &skill.tool_definitions {
            let tool_name = &td.function.name;
            let formatted = match format_str {
                "claude" => td.to_claude_format(),
                _ => serde_json::to_value(td).unwrap_or_default(),
            };
            tools.push(formatted);
            let script_path = skill.multi_script_entries.get(tool_name).cloned();
            let entry_point = if script_path.is_none() && !skill.metadata.entry_point.is_empty() {
                Some(skill.metadata.entry_point.clone())
            } else {
                script_path.clone()
            };
            let is_bash = skill.metadata.is_bash_tool_skill();
            tool_meta.insert(
                tool_name.clone(),
                json!({
                    "skill_dir": skill_dir_str,
                    "script_path": script_path,
                    "entry_point": entry_point,
                    "is_bash": is_bash
                }),
            );
        }
    }

    Ok(json!({ "tools": tools, "tool_meta": tool_meta }))
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
    if std::env::var("SKILLLITE_OUTPUT_DIR").is_err() {
        let chat_output = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output");
        unsafe { std::env::set_var("SKILLLITE_OUTPUT_DIR", chat_output.to_string_lossy().as_ref()) };
    }

    // Ensure output directory exists so skills can write files there immediately
    if let Ok(output_dir) = std::env::var("SKILLLITE_OUTPUT_DIR") {
        let p = PathBuf::from(&output_dir);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }

    // Enable task planning: --plan > --no-plan > env var > default (true)
    config.enable_task_planning = if plan {
        true
    } else if no_plan {
        false
    } else {
        std::env::var("SKILLLITE_ENABLE_TASK_PLANNING")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(true) // Default to true when skills are available
    };

    // Validate API key
    if config.api_key.is_empty() {
        anyhow::bail!(
            "API key required. Set OPENAI_API_KEY env var or use --api-key flag."
        );
    }

    // Auto-discover skill directories if none specified
    let effective_skill_dirs = if skill_dirs.is_empty() {
        let ws = Path::new(&config.workspace);
        let mut auto_dirs = Vec::new();
        // Check common skill directory names
        for name in &[".skills", "skills"] {
            let dir = ws.join(name);
            if dir.is_dir() {
                // Each subdirectory is a skill
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.join("SKILL.md").exists() {
                            auto_dirs.push(path.to_string_lossy().to_string());
                        }
                    }
                }
                if auto_dirs.is_empty() {
                    // Might be a flat structure — add the directory itself
                    auto_dirs.push(dir.to_string_lossy().to_string());
                }
            }
        }
        if !auto_dirs.is_empty() {
            eprintln!("🔍 Auto-discovered {} skill(s) in workspace", auto_dirs.len());
        }
        auto_dirs
    } else {
        skill_dirs
    };

    // Load skills
    let loaded_skills = skills::load_skills(&effective_skill_dirs);
    if !loaded_skills.is_empty() {
        eprintln!("📦 Loaded {} skill(s):", loaded_skills.len());
        for s in &loaded_skills {
            eprintln!("   - {}", s.name);
        }
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

#[cfg(feature = "agent")]
async fn run_interactive_chat(
    config: agent::types::AgentConfig,
    session_key: &str,
    skills: Vec<agent::skills::LoadedSkill>,
    verbose: bool,
) -> Result<()> {
    use agent::types::*;
    use agent::chat_session::ChatSession;

    eprintln!("🤖 SkillBox Chat (model: {})", config.model);
    eprintln!("   Type /exit to quit, /clear to reset, /compact to compress history\n");

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
                        // Compaction happens automatically in run_turn
                        continue;
                    }
                    _ => {}
                }

                // Run the turn
                eprint!("\nAssistant> ");
                match session.run_turn(input, &mut sink).await {
                    Ok(_response) => {
                        eprintln!(); // newline after streaming
                    }
                    Err(e) => {
                        eprintln!("\n❌ Error: {}", e);
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("^C");
                continue;
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
