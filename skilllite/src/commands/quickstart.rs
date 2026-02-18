//! `skilllite quickstart` — Zero-config start: LLM detection + skills setup + chat launch.
//!
//! Migrated from Python `skilllite quickstart` command.
//!
//! Flow:
//!   1. LLM setup:
//!      - Priority 1: Existing .env with valid OPENAI_API_KEY
//!      - Priority 2: Auto-detect Ollama (GET http://localhost:11434/api/tags)
//!      - Priority 3: Interactive provider selection
//!   2. Ensure skills are available
//!   3. Start interactive chat

use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Provider options for LLM selection.
#[derive(Debug, Clone)]
struct LlmProvider {
    name: &'static str,
    api_base: &'static str,
    env_key: &'static str,
    needs_key: bool,
}

const PROVIDERS: &[LlmProvider] = &[
    LlmProvider {
        name: "Ollama (local, free)",
        api_base: "http://localhost:11434/v1",
        env_key: "",
        needs_key: false,
    },
    LlmProvider {
        name: "OpenAI",
        api_base: "https://api.openai.com/v1",
        env_key: "OPENAI_API_KEY",
        needs_key: true,
    },
    LlmProvider {
        name: "DeepSeek",
        api_base: "https://api.deepseek.com/v1",
        env_key: "DEEPSEEK_API_KEY",
        needs_key: true,
    },
    LlmProvider {
        name: "Qwen (Alibaba Cloud)",
        api_base: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        env_key: "DASHSCOPE_API_KEY",
        needs_key: true,
    },
];

const DEFAULT_MODELS: &[(&str, &str)] = &[
    ("localhost:11434", "qwen2.5:7b"),
    ("api.openai.com", "gpt-4o"),
    ("api.deepseek.com", "deepseek-chat"),
    ("dashscope.aliyuncs.com", "qwen-plus"),
];

/// `skilllite quickstart`
pub fn cmd_quickstart(skills_dir: &str) -> Result<()> {
    let skills_path = resolve_path(skills_dir);

    eprintln!("🚀 SkillLite Quickstart");
    eprintln!("   Zero-config setup — let's get you chatting with AI skills!");
    eprintln!();

    // Step 1: LLM setup
    let (api_base, api_key, model) = setup_llm()?;
    eprintln!();

    // Step 2: Ensure skills
    ensure_skills(&skills_path)?;
    eprintln!();

    // Step 3: Write .env if it doesn't exist (so future runs remember)
    write_env_if_needed(&api_base, &api_key, &model)?;

    // Step 4: Launch chat
    eprintln!("🤖 Starting chat...");
    eprintln!();

    launch_chat(&api_base, &api_key, &model, &skills_path)
}

fn resolve_path(dir: &str) -> PathBuf {
    let p = PathBuf::from(dir);
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// Step 1: Detect or interactively set up LLM configuration.
/// Returns (api_base, api_key, model).
fn setup_llm() -> Result<(String, String, String)> {
    eprintln!("🔍 Step 1/3: Detecting LLM configuration...");

    // Priority 1: Check existing .env / environment variables
    if let Some(config) = detect_existing_config() {
        eprintln!("   ✅ Found existing configuration:");
        eprintln!("      API Base: {}", config.0);
        eprintln!("      Model: {}", config.2);
        return Ok(config);
    }

    // Priority 2: Probe Ollama
    if let Some(config) = probe_ollama() {
        eprintln!("   ✅ Detected local Ollama instance:");
        eprintln!("      Model: {}", config.2);
        return Ok(config);
    }

    // Priority 3: Interactive selection
    eprintln!("   No LLM configuration found. Let's set one up!");
    eprintln!();
    interactive_llm_setup()
}

/// Check if .env or environment variables already have valid LLM config.
fn detect_existing_config() -> Option<(String, String, String)> {
    // Load .env if it exists
    load_dotenv();

    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("SKILLLITE_API_KEY"))
        .ok()
        .filter(|k| !k.is_empty() && k != "sk-xxx")?;

    let api_base = std::env::var("OPENAI_API_BASE")
        .or_else(|_| std::env::var("OPENAI_BASE_URL"))
        .or_else(|_| std::env::var("SKILLLITE_API_BASE"))
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

    let model = std::env::var("SKILLLITE_MODEL")
        .or_else(|_| std::env::var("OPENAI_MODEL"))
        .unwrap_or_else(|_| default_model_for_base(&api_base).to_string());

    Some((api_base, api_key, model))
}

/// Load .env file from current directory.
fn load_dotenv() {
    let env_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".env");

    if let Ok(content) = fs::read_to_string(&env_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                // Only set if not already set in real env
                if std::env::var(key).is_err() {
                    // SAFETY: quickstart runs at startup before tokio runtime,
                    // single-threaded context.
                    unsafe { std::env::set_var(key, value) };
                }
            }
        }
    }
}

/// Probe Ollama at localhost:11434.
fn probe_ollama() -> Option<(String, String, String)> {
    eprintln!("   Probing Ollama at localhost:11434...");

    // Use a blocking HTTP GET with short timeout
    let url = "http://localhost:11434/api/tags";
    let output = std::process::Command::new("curl")
        .args(["-s", "--connect-timeout", "2", "--max-time", "3", url])
        .output()
        .ok()?;

    if !output.status.success() {
        eprintln!("   → Ollama not detected");
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let models = json.get("models")?.as_array()?;

    if models.is_empty() {
        eprintln!("   → Ollama found but no models installed");
        eprintln!("     Run: ollama pull qwen2.5:7b");
        return None;
    }

    // Pick best available model
    let preferred = [
        "qwen2.5:7b",
        "qwen2.5:14b",
        "llama3.1:8b",
        "llama3.1:70b",
        "deepseek-coder-v2:16b",
        "codellama:7b",
        "mistral:7b",
    ];

    let model_names: Vec<String> = models
        .iter()
        .filter_map(|m| m.get("name")?.as_str().map(|s| s.to_string()))
        .collect();

    let selected = preferred
        .iter()
        .find(|p| model_names.iter().any(|m| m.starts_with(*p)))
        .map(|s| s.to_string())
        .unwrap_or_else(|| model_names.first().cloned().unwrap_or_else(|| "qwen2.5:7b".to_string()));

    eprintln!(
        "   → Ollama detected with {} model(s), using: {}",
        model_names.len(),
        selected
    );

    Some((
        "http://localhost:11434/v1".to_string(),
        "ollama".to_string(), // Ollama doesn't need a real key, but the field can't be empty
        selected,
    ))
}

/// Interactive provider selection.
fn interactive_llm_setup() -> Result<(String, String, String)> {
    eprintln!("   Select LLM provider:");
    for (i, provider) in PROVIDERS.iter().enumerate() {
        eprintln!("     {}. {}", i + 1, provider.name);
    }
    eprintln!("     {}. Custom (enter your own URL)", PROVIDERS.len() + 1);
    eprintln!();

    eprint!("   Choice [1]: ");
    std::io::stderr().flush()?;
    let mut choice = String::new();
    std::io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();
    let idx: usize = if choice.is_empty() {
        0
    } else {
        choice.parse::<usize>().unwrap_or(1).saturating_sub(1)
    };

    if idx < PROVIDERS.len() {
        let provider = &PROVIDERS[idx];
        let api_base = provider.api_base.to_string();

        let api_key = if provider.needs_key {
            // Check env first
            if let Ok(key) = std::env::var(provider.env_key) {
                if !key.is_empty() && key != "sk-xxx" {
                    eprintln!("   ✅ Using {} from environment", provider.env_key);
                    key
                } else {
                    prompt_api_key(provider.env_key)?
                }
            } else {
                prompt_api_key(provider.env_key)?
            }
        } else {
            "ollama".to_string()
        };

        let model = default_model_for_base(&api_base).to_string();
        eprintln!("   Model: {} (change via SKILLLITE_MODEL env var)", model);

        Ok((api_base, api_key, model))
    } else {
        // Custom
        eprint!("   API Base URL: ");
        std::io::stderr().flush()?;
        let mut api_base = String::new();
        std::io::stdin().read_line(&mut api_base)?;
        let api_base = api_base.trim().to_string();

        let api_key = prompt_api_key("API_KEY")?;

        eprint!("   Model name: ");
        std::io::stderr().flush()?;
        let mut model = String::new();
        std::io::stdin().read_line(&mut model)?;
        let model = model.trim().to_string();

        Ok((api_base, api_key, model))
    }
}

fn prompt_api_key(env_var_name: &str) -> Result<String> {
    eprint!("   {} (or set {} env var): ", "API Key", env_var_name);
    std::io::stderr().flush()?;
    let mut key = String::new();
    std::io::stdin().read_line(&mut key)?;
    let key = key.trim().to_string();
    if key.is_empty() {
        anyhow::bail!("API key is required for this provider");
    }
    Ok(key)
}

fn default_model_for_base(api_base: &str) -> &str {
    for (host, model) in DEFAULT_MODELS {
        if api_base.contains(host) {
            return model;
        }
    }
    "gpt-4o"
}

/// Step 2: Ensure skills are available.
fn ensure_skills(skills_path: &Path) -> Result<()> {
    eprintln!("📦 Step 2/3: Checking skills...");

    if skills_path.is_dir() {
        let skill_count = fs::read_dir(skills_path)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
                    .count()
            })
            .unwrap_or(0);

        if skill_count > 0 {
            eprintln!("   ✅ Found {} skill(s) in {}", skill_count, skills_path.display());
            return Ok(());
        }
    }

    // Check SKILLLITE_SKILLS_REPO for remote skills
    if let Ok(repo) = std::env::var("SKILLLITE_SKILLS_REPO") {
        if !repo.is_empty() {
            eprintln!("   📥 Cloning skills from SKILLLITE_SKILLS_REPO: {}", repo);
            // Use the add command to install
            crate::commands::skill::cmd_add(&repo, &skills_path.to_string_lossy(), false, false)?;
            return Ok(());
        }
    }

    // No skills available — create example
    eprintln!("   No skills found. Creating example skill...");
    crate::commands::init::cmd_init(&skills_path.to_string_lossy(), true, true, false, false)?;

    Ok(())
}

/// Write .env file if it doesn't already exist.
fn write_env_if_needed(api_base: &str, api_key: &str, model: &str) -> Result<()> {
    let env_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".env");

    if env_path.exists() {
        return Ok(());
    }

    // Don't write .env if using Ollama (no sensitive key)
    if api_key == "ollama" {
        // Still set env vars for this session
        // SAFETY: Called before tokio runtime, single-threaded.
        unsafe {
            std::env::set_var("OPENAI_API_BASE", api_base);
            std::env::set_var("OPENAI_API_KEY", api_key);
            std::env::set_var("SKILLLITE_MODEL", model);
        }
        return Ok(());
    }

    let content = format!(
        "# SkillLite LLM Configuration (auto-generated by quickstart)\n\
         OPENAI_API_BASE={}\n\
         OPENAI_API_KEY={}\n\
         SKILLLITE_MODEL={}\n",
        api_base, api_key, model
    );

    fs::write(&env_path, &content)
        .with_context(|| format!("Failed to write .env file: {}", env_path.display()))?;

    eprintln!("💾 Saved configuration to .env");
    eprintln!("   ⚠ Add .env to .gitignore to avoid leaking your API key!");

    Ok(())
}

/// Step 3: Launch the chat session.
#[cfg(feature = "agent")]
fn launch_chat(api_base: &str, api_key: &str, model: &str, skills_path: &Path) -> Result<()> {
    use crate::agent::types::*;
    use crate::agent::skills;

    // Set environment variables for the agent
    // SAFETY: Called before tokio runtime, single-threaded.
    unsafe {
        std::env::set_var("OPENAI_API_BASE", api_base);
        std::env::set_var("OPENAI_API_KEY", api_key);
        std::env::set_var("SKILLLITE_MODEL", model);
    }

    let mut config = AgentConfig::from_env();
    config.api_base = api_base.to_string();
    config.api_key = api_key.to_string();
    config.model = model.to_string();
    config.enable_task_planning = true;

    // Discover skills
    let mut skill_dirs = Vec::new();
    if skills_path.is_dir() {
        if let Ok(entries) = fs::read_dir(skills_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    skill_dirs.push(p.to_string_lossy().to_string());
                }
            }
        }
    }

    let loaded_skills = skills::load_skills(&skill_dirs);
    if !loaded_skills.is_empty() {
        eprintln!("📦 Loaded {} skill(s):", loaded_skills.len());
        for s in &loaded_skills {
            eprintln!("   - {}", s.name);
        }
    }
    eprintln!();

    let rt = tokio::runtime::Runtime::new()
        .context("Failed to create tokio runtime")?;

    rt.block_on(async {
        run_interactive_quickstart(config, loaded_skills).await
    })
}

#[cfg(feature = "agent")]
async fn run_interactive_quickstart(
    config: crate::agent::types::AgentConfig,
    skills: Vec<crate::agent::skills::LoadedSkill>,
) -> Result<()> {
    use crate::agent::types::*;
    use crate::agent::chat_session::ChatSession;

    eprintln!("🤖 SkillBox Quickstart Chat (model: {})", config.model);
    eprintln!("   Type /exit to quit, /clear to reset, /compact to compress history");
    eprintln!();

    let mut session = ChatSession::new(config, "quickstart", skills);
    let mut sink = TerminalEventSink::new(false);

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
                        continue;
                    }
                    _ => {}
                }

                eprint!("\nAssistant> ");
                match session.run_turn(input, &mut sink).await {
                    Ok(_) => {
                        eprintln!();
                    }
                    Err(e) => {
                        eprintln!("\n❌ Error: {}", e);
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

#[cfg(not(feature = "agent"))]
fn launch_chat(_api_base: &str, _api_key: &str, _model: &str, _skills_path: &Path) -> Result<()> {
    anyhow::bail!(
        "The `agent` feature is required for quickstart chat.\n\
         Rebuild with: cargo build --features agent"
    )
}
