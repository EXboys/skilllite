//! CLI chat entry-points: single-shot and interactive REPL.
//!
//! Extracted from `main.rs` so that `main` only does argument dispatch.

use anyhow::{Context, Result};
use std::path::Path;

use super::chat_session::ChatSession;
use super::skills;
use super::types::*;

/// Run a single task (one turn) and return AgentResult.
/// Used by P2P swarm when routing decides to execute locally.
///
/// When `skill_dirs` is `Some`, loads skills from those directories (e.g. swarm's `--skills-dir`).
/// When `None`, auto-discovers from `workspace/.skills` and `workspace/skills`.
pub async fn run_single_task(
    workspace: &str,
    session_key: &str,
    description: &str,
    skill_dirs: Option<&[String]>,
) -> Result<AgentResult> {
    let mut config = AgentConfig::from_env();
    config.workspace = workspace.to_string();
    config.enable_task_planning = false; // Single task, no planning
    config.enable_memory = true;

    if config.api_key.is_empty() {
        anyhow::bail!("API key required for swarm task execution. Set OPENAI_API_KEY.");
    }

    skilllite_core::config::ensure_default_output_dir();

    let skill_dirs = skill_dirs
        .map(|s| s.to_vec())
        .unwrap_or_else(|| {
            skilllite_core::skill::discovery::discover_skill_dirs_for_loading(
                Path::new(workspace),
                Some(&[".skills", "skills"]),
            )
        });
    let loaded_skills = skills::load_skills(&skill_dirs);

    let mut session = ChatSession::new(config, session_key, loaded_skills);
    let mut sink = SilentEventSink;
    session.run_turn(description, &mut sink).await
}

/// Clear session (OpenClaw-style): summarize to memory, archive transcript, reset counts.
/// Called by `skilllite clear-session` and Assistant. Loads .env from workspace.
pub fn run_clear_session(session_key: &str, workspace: &str) -> Result<()> {
    let workspace_path = Path::new(workspace).canonicalize().unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(workspace)
    });
    if std::env::set_current_dir(&workspace_path).is_err() {
        // Non-fatal: .env may not exist or API key may be in env already
    }

    let mut config = AgentConfig::from_env();
    config.workspace = workspace_path.to_string_lossy().to_string();

    if config.api_key.is_empty() {
        tracing::warn!(
            "No OPENAI_API_KEY; summarization skipped. Session will still be archived and counts reset."
        );
    }

    skilllite_core::config::ensure_default_output_dir();

    let loaded_skills = skills::load_skills(&[]);
    let mut session = ChatSession::new(config, session_key, loaded_skills);

    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    rt.block_on(async { session.clear_full().await })?;

    Ok(())
}

/// Top-level entry-point called from `main()` for the `chat` subcommand.
pub fn run_chat(
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
    soul_path: Option<String>,
) -> Result<()> {
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
    config.soul_path = soul_path;

    skilllite_core::config::ensure_default_output_dir();

    // Enable task planning: --plan > --no-plan > config (default true)
    if plan {
        config.enable_task_planning = true;
    } else if no_plan {
        config.enable_task_planning = false;
    }

    config.enable_memory = !no_memory;

    if config.api_key.is_empty() {
        anyhow::bail!(
            "API key required. Set OPENAI_API_KEY env var or use --api-key flag."
        );
    }

    // Auto-discover skill directories if none specified
    let (effective_skill_dirs, was_auto_discovered) = if skill_dirs.is_empty() {
        let auto_dirs = skilllite_core::skill::discovery::discover_skill_dirs_for_loading(
            Path::new(&config.workspace),
            Some(&[".skills", "skills"]),
        );
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

    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    if let Some(msg) = single_message {
        rt.block_on(async {
            let mut session = ChatSession::new(config, &session_key, loaded_skills);
            let mut sink = TerminalEventSink::new(verbose);
            let result = session.run_turn(&msg, &mut sink).await?;
            println!("\n{}", result.response);
            Ok(())
        })
    } else {
        rt.block_on(async {
            run_interactive_chat(config, &session_key, loaded_skills, verbose).await
        })
    }
}

/// Run agent in unattended mode: one-time goal, continuous execution until done/timeout.
/// Replan (update_task_plan) does not wait for user — agent continues immediately.
/// Confirmations (run_command, L3 skill scan) are auto-approved.
/// A13: When resume=true, load checkpoint and continue from last state.
pub fn run_agent_run(
    api_base: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    workspace: Option<String>,
    skill_dirs: Vec<String>,
    soul_path: Option<String>,
    goal: String,
    max_iterations: usize,
    verbose: bool,
    max_failures: Option<usize>,
    resume: bool,
) -> Result<()> {
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
    config.soul_path = soul_path;
    config.verbose = verbose;
    config.enable_task_planning = true;
    config.enable_memory = true;
    // A4: Failure retry limit — prevents infinite loops on repeated failures
    config.max_consecutive_failures = match max_failures {
        Some(0) => None,       // 0 = no limit
        Some(n) => Some(n),
        None => Some(5),       // default: stop after 5 consecutive failures
    };
    // A5: Goal boundaries extracted in agent_loop (hybrid: regex + optional LLM fallback)

    if config.api_key.is_empty() {
        anyhow::bail!(
            "API key required. Set OPENAI_API_KEY env var or use --api-key flag."
        );
    }

    skilllite_core::config::ensure_default_output_dir();

    // A13: Resume from checkpoint
    let (effective_goal, effective_workspace, history_override) = if resume {
        let chat_root = skilllite_executor::chat_root();
        match super::run_checkpoint::load_checkpoint(&chat_root)? {
            Some(cp) => {
                let resume_msg = super::run_checkpoint::build_resume_message(&cp);
                // Use checkpoint messages as history; skip first (system) since agent_loop adds its own
                let history: Vec<ChatMessage> = cp
                    .messages
                    .into_iter()
                    .skip(1)
                    .collect();
                eprintln!("📂 从断点续跑 (run_id: {})", cp.run_id);
                (resume_msg, cp.workspace, Some(history))
            }
            None => {
                anyhow::bail!("无可用断点。请先运行 `skilllite run --goal \"...\"` 以创建断点。");
            }
        }
    } else {
        (goal, config.workspace.clone(), None)
    };

    config.workspace = effective_workspace;

    let (effective_skill_dirs, was_auto_discovered) = if skill_dirs.is_empty() {
        let auto_dirs = skilllite_core::skill::discovery::discover_skill_dirs_for_loading(
            Path::new(&config.workspace),
            Some(&[".skills", "skills"]),
        );
        let has_skills = !auto_dirs.is_empty();
        (auto_dirs, has_skills)
    } else {
        (skill_dirs, false)
    };

    let loaded_skills = skills::load_skills(&effective_skill_dirs);
    if !loaded_skills.is_empty() {
        eprintln!("┌─ Run mode ───────────────────────────────────────────────");
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
        eprintln!("│  🎯 Goal: {}", effective_goal.lines().next().unwrap_or(&effective_goal));
        eprintln!("└───────────────────────────────────────────────────────────\n");
    }

    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    let history_override = history_override;
    rt.block_on(async {
        let mut session = ChatSession::new(config, "run", loaded_skills);
        let mut sink = RunModeEventSink::new(verbose);
        let result = if let Some(history) = history_override {
            session.run_turn_with_history(&effective_goal, &mut sink, history).await
        } else {
            session.run_turn(&effective_goal, &mut sink).await
        };
        let _ = result?;
        // Response already streamed via sink during run_turn — no extra println
        Ok(())
    })
}

/// Format agent/API errors for user-friendly display in chat UI.
fn format_chat_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
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
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s
    }
}

async fn run_interactive_chat(
    config: AgentConfig,
    session_key: &str,
    skills: Vec<skills::LoadedSkill>,
    verbose: bool,
) -> Result<()> {
    eprintln!("┌────────────────────────────────────────────────────────────");
    eprintln!("│  🤖 SkillBox Chat  ·  model: {}", config.model);
    eprintln!("│  /exit 退出  ·  /clear 清空  ·  /compact 压缩历史");
    eprintln!("└────────────────────────────────────────────────────────────\n");

    let mut session = ChatSession::new(config, session_key, skills);
    let mut sink = TerminalEventSink::new(verbose);

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

                let _ = rl.add_history_entry(input);

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
