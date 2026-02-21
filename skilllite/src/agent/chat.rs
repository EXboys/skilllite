//! CLI chat entry-points: single-shot and interactive REPL.
//!
//! Extracted from `main.rs` so that `main` only does argument dispatch.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::chat_session::ChatSession;
use super::skills;
use super::types::*;

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

    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    if let Some(msg) = single_message {
        rt.block_on(async {
            let mut session = ChatSession::new(config, &session_key, loaded_skills);
            let mut sink = TerminalEventSink::new(verbose);
            let response = session.run_turn(&msg, &mut sink).await?;
            println!("\n{}", response);
            Ok(())
        })
    } else {
        rt.block_on(async {
            run_interactive_chat(config, &session_key, loaded_skills, verbose).await
        })
    }
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
