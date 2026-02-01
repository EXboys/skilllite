mod cli;
mod config;
mod env;
// mod protocol;
mod sandbox;
mod skill;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            skill_dir,
            input_json,
            allow_network,
            cache_dir,
            max_memory,
            timeout,
            sandbox_level,
        } => {
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = run_skill(&skill_dir, &input_json, allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
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
            let sandbox_level = sandbox::executor::SandboxLevel::from_env_or_cli(sandbox_level);
            let limits = sandbox::executor::ResourceLimits::from_env()
                .with_cli_overrides(max_memory, timeout);
            let result = exec_script(&skill_dir, &script_path, &input_json, args.as_ref(), allow_network, cache_dir.as_ref(), limits, sandbox_level)?;
            println!("{}", result);
        }
        Commands::Scan {
            skill_dir,
            preview_lines,
        } => {
            let result = scan_skill(&skill_dir, preview_lines)?;
            println!("{}", result);
        }
        Commands::Validate { skill_dir } => {
            validate_skill(&skill_dir)?;
            println!("Skill validation passed!");
        }
        Commands::Info { skill_dir } => {
            show_skill_info(&skill_dir)?;
        }
        Commands::SecurityScan {
            script_path,
            allow_network,
            allow_file_ops,
            allow_process_exec,
        } => {
            security_scan_script(&script_path, allow_network, allow_file_ops, allow_process_exec)?;
        }
    }

    Ok(())
}

fn run_skill(
    skill_dir: &str,
    input_json: &str,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: sandbox::executor::ResourceLimits,
    sandbox_level: sandbox::executor::SandboxLevel,
) -> Result<String> {
    use std::path::Path;

    let skill_path = Path::new(skill_dir);

    // 1. Parse SKILL.md metadata
    let metadata = skill::metadata::parse_skill_metadata(skill_path)?;

    // Check if this is a prompt-only skill (no entry_point)
    if metadata.entry_point.is_empty() {
        anyhow::bail!("This skill has no entry point and cannot be executed. It is a prompt-only skill.");
    }

    // 2. Validate input JSON
    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    // 3. Setup environment (venv/node_modules)
    let env_path = env::builder::ensure_environment(skill_path, &metadata, cache_dir.map(|s| s.as_str()))?;

    // 4. Apply CLI overrides and execute in sandbox
    let mut effective_metadata = metadata;
    
    // CLI --allow-network flag takes precedence
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    let output = sandbox::executor::run_in_sandbox_with_limits_and_level(
        skill_path,
        &env_path,
        &effective_metadata,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

fn validate_skill(skill_dir: &str) -> Result<()> {
    let skill_path = std::path::Path::new(skill_dir);

    // Parse and validate metadata
    let metadata = skill::metadata::parse_skill_metadata(skill_path)?;

    // Check entry point exists (only if entry_point is specified)
    if !metadata.entry_point.is_empty() {
        let entry_path = skill_path.join(&metadata.entry_point);
        if !entry_path.exists() {
            anyhow::bail!("Entry point not found: {}", metadata.entry_point);
        }

        // Check dependencies file if language specified
        skill::deps::validate_dependencies(skill_path, &metadata)?;
    }

    Ok(())
}

fn show_skill_info(skill_dir: &str) -> Result<()> {
    use std::path::Path;

    let skill_path = Path::new(skill_dir);
    let metadata = skill::metadata::parse_skill_metadata(skill_path)?;

    println!("Skill Information:");
    println!("  Name: {}", metadata.name);
    if metadata.entry_point.is_empty() {
        println!("  Entry Point: (none - prompt-only skill)");
    } else {
        println!("  Entry Point: {}", metadata.entry_point);
    }
    println!(
        "  Language: {}",
        metadata.language.as_deref().unwrap_or("auto-detect")
    );
    println!("  Network Enabled: {}", metadata.network.enabled);
    if !metadata.network.outbound.is_empty() {
        println!("  Outbound Whitelist:");
        for host in &metadata.network.outbound {
            println!("    - {}", host);
        }
    }

    Ok(())
}

/// Execute a specific script directly in sandbox
fn exec_script(
    skill_dir: &str,
    script_path: &str,
    input_json: &str,
    args: Option<&String>,
    allow_network: bool,
    cache_dir: Option<&String>,
    limits: sandbox::executor::ResourceLimits,
    sandbox_level: sandbox::executor::SandboxLevel,
) -> Result<String> {
    use std::path::Path;

    let skill_path = Path::new(skill_dir);
    let full_script_path = skill_path.join(script_path);

    // Validate script exists
    if !full_script_path.exists() {
        anyhow::bail!("Script not found: {}", full_script_path.display());
    }

    // Detect language from script extension
    let language = detect_script_language(&full_script_path)?;

    // Validate input JSON
    let _input: serde_json::Value = serde_json::from_str(input_json)
        .map_err(|e| anyhow::anyhow!("Invalid input JSON: {}", e))?;

    // Try to parse SKILL.md for network policy and dependencies (optional)
    let (metadata, env_path) = if skill_path.join("SKILL.md").exists() {
        let mut meta = skill::metadata::parse_skill_metadata(skill_path)?;
        // Override entry_point with the specified script
        meta.entry_point = script_path.to_string();
        meta.language = Some(language.clone());
        
        // Setup environment based on skill dependencies
        let env = env::builder::ensure_environment(skill_path, &meta, cache_dir.map(|s| s.as_str()))?;
        (meta, env)
    } else {
        // No SKILL.md, create minimal metadata
        let meta = skill::metadata::SkillMetadata {
            name: skill_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            entry_point: script_path.to_string(),
            language: Some(language.clone()),
            description: None,
            compatibility: None,
            network: skill::metadata::NetworkPolicy::default(),
        };
        (meta, std::path::PathBuf::new())
    };

    // Apply network settings
    let mut effective_metadata = metadata;
    if allow_network {
        effective_metadata.network.enabled = true;
    }

    // Store args in metadata for the executor to use
    if let Some(ref args_str) = args {
        // Parse args and pass them through environment variable
        // SAFETY: We are setting an environment variable before spawning any threads
        unsafe {
            std::env::set_var("SKILLBOX_SCRIPT_ARGS", args_str);
        }
    }

    // Execute in sandbox
    let output = sandbox::executor::run_in_sandbox_with_limits_and_level(
        skill_path,
        &env_path,
        &effective_metadata,
        input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

/// Detect script language from file extension
fn detect_script_language(script_path: &std::path::Path) -> Result<String> {
    let extension = script_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match extension {
        "py" => Ok("python".to_string()),
        "js" | "mjs" | "cjs" => Ok("node".to_string()),
        "ts" => Ok("node".to_string()),
        "sh" | "bash" => Ok("shell".to_string()),
        "" => {
            // Check shebang for scripts without extension
            if let Ok(content) = std::fs::read_to_string(script_path) {
                if let Some(first_line) = content.lines().next() {
                    if first_line.starts_with("#!") {
                        if first_line.contains("python") {
                            return Ok("python".to_string());
                        } else if first_line.contains("node") {
                            return Ok("node".to_string());
                        } else if first_line.contains("bash") || first_line.contains("sh") {
                            return Ok("shell".to_string());
                        }
                    }
                }
            }
            anyhow::bail!("Cannot detect language for script: {}", script_path.display())
        }
        _ => anyhow::bail!("Unsupported script extension: .{}", extension),
    }
}

/// Perform security scan on a script
fn security_scan_script(
    script_path: &str,
    allow_network: bool,
    allow_file_ops: bool,
    allow_process_exec: bool,
) -> Result<()> {
    use std::path::Path;
    use crate::sandbox::security::{ScriptScanner, format_scan_result};

    let path = Path::new(script_path);
    
    // Validate script exists
    if !path.exists() {
        anyhow::bail!("Script not found: {}", path.display());
    }

    // Create scanner with specified permissions
    let scanner = ScriptScanner::new()
        .allow_network(allow_network)
        .allow_file_ops(allow_file_ops)
        .allow_process_exec(allow_process_exec);

    // Scan the script
    let scan_result = scanner.scan_file(path)?;

    // Display results
    println!("Security Scan Results for: {}\n", path.display());
    println!("{}", format_scan_result(&scan_result));

    Ok(())
}

/// Scan skill directory and return JSON with all executable scripts
fn scan_skill(skill_dir: &str, preview_lines: usize) -> Result<String> {
    use std::path::Path;

    let skill_path = Path::new(skill_dir);

    if !skill_path.exists() {
        anyhow::bail!("Skill directory not found: {}", skill_dir);
    }

    let mut result = serde_json::json!({
        "skill_dir": skill_dir,
        "has_skill_md": false,
        "skill_metadata": null,
        "scripts": [],
        "directories": {
            "scripts": false,
            "references": false,
            "assets": false
        }
    });

    // Check for SKILL.md and parse metadata
    let skill_md_path = skill_path.join("SKILL.md");
    if skill_md_path.exists() {
        result["has_skill_md"] = serde_json::json!(true);
        if let Ok(metadata) = skill::metadata::parse_skill_metadata(skill_path) {
            result["skill_metadata"] = serde_json::json!({
                "name": metadata.name,
                "description": metadata.description,
                "entry_point": if metadata.entry_point.is_empty() { None } else { Some(&metadata.entry_point) },
                "language": metadata.language,
                "network_enabled": metadata.network.enabled,
                "compatibility": metadata.compatibility
            });
        }
    }

    // Check standard directories
    result["directories"]["scripts"] = serde_json::json!(skill_path.join("scripts").exists());
    result["directories"]["references"] = serde_json::json!(skill_path.join("references").exists());
    result["directories"]["assets"] = serde_json::json!(skill_path.join("assets").exists());

    // Scan for executable scripts
    let mut scripts = Vec::new();
    scan_scripts_recursive(skill_path, skill_path, &mut scripts, preview_lines)?;

    result["scripts"] = serde_json::json!(scripts);

    serde_json::to_string_pretty(&result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize scan result: {}", e))
}

/// Recursively scan for executable scripts
fn scan_scripts_recursive(
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    scripts: &mut Vec<serde_json::Value>,
    preview_lines: usize,
) -> Result<()> {
    use std::fs;

    let entries = fs::read_dir(current_path)
        .map_err(|e| anyhow::anyhow!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and directories
        if file_name.starts_with('.') {
            continue;
        }

        // Skip common non-script directories
        if path.is_dir() {
            let skip_dirs = ["node_modules", "__pycache__", ".git", "venv", ".venv", "assets", "references"];
            if skip_dirs.contains(&file_name.as_str()) {
                continue;
            }
            scan_scripts_recursive(base_path, &path, scripts, preview_lines)?;
            continue;
        }

        // Check if it's an executable script
        if let Some(script_info) = analyze_script_file(&path, base_path, preview_lines) {
            scripts.push(script_info);
        }
    }

    Ok(())
}

/// Analyze a single script file and return its metadata
fn analyze_script_file(
    file_path: &std::path::Path,
    base_path: &std::path::Path,
    preview_lines: usize,
) -> Option<serde_json::Value> {
    use std::fs;

    let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    
    // Supported script extensions
    let (language, is_script) = match extension {
        "py" => ("python", true),
        "js" | "mjs" | "cjs" => ("node", true),
        "ts" => ("typescript", true),
        "sh" | "bash" => ("shell", true),
        "" => {
            // Check shebang for files without extension
            if let Ok(content) = fs::read_to_string(file_path) {
                if let Some(first_line) = content.lines().next() {
                    if first_line.starts_with("#!") {
                        if first_line.contains("python") {
                            ("python", true)
                        } else if first_line.contains("node") {
                            ("node", true)
                        } else if first_line.contains("bash") || first_line.contains("sh") {
                            ("shell", true)
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };

    if !is_script {
        return None;
    }

    let relative_path = file_path.strip_prefix(base_path).ok()?;
    let content = fs::read_to_string(file_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Extract preview (first N lines)
    let preview: String = lines.iter()
        .take(preview_lines)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    // Extract docstring/description
    let description = extract_script_description(&content, language);

    // Detect if script has main entry point
    let has_main = detect_main_entry(&content, language);

    // Detect CLI arguments usage
    let uses_argparse = detect_argparse_usage(&content, language);

    // Detect stdin/stdout usage
    let uses_stdio = detect_stdio_usage(&content, language);

    Some(serde_json::json!({
        "path": relative_path.to_string_lossy(),
        "language": language,
        "total_lines": total_lines,
        "preview": preview,
        "description": description,
        "has_main_entry": has_main,
        "uses_argparse": uses_argparse,
        "uses_stdio": uses_stdio,
        "file_size_bytes": fs::metadata(file_path).map(|m| m.len()).unwrap_or(0)
    }))
}

/// Extract script description from docstring or comments
fn extract_script_description(content: &str, language: &str) -> Option<String> {
    match language {
        "python" => {
            // Look for module docstring
            let trimmed = content.trim_start();
            if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
                let quote = if trimmed.starts_with("\"\"\"") { "\"\"\"" } else { "'''" };
                if let Some(start) = trimmed.find(quote) {
                    let rest = &trimmed[start + 3..];
                    if let Some(end) = rest.find(quote) {
                        return Some(rest[..end].trim().to_string());
                    }
                }
            }
            // Look for leading comments
            let mut desc_lines = Vec::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('#') && !trimmed.starts_with("#!") {
                    desc_lines.push(trimmed.trim_start_matches('#').trim());
                } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        "node" | "typescript" => {
            // Look for JSDoc or leading comments
            let trimmed = content.trim_start();
            if trimmed.starts_with("/**") {
                if let Some(end) = trimmed.find("*/") {
                    let doc = &trimmed[3..end];
                    let cleaned: Vec<&str> = doc.lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty())
                        .collect();
                    return Some(cleaned.join(" "));
                }
            }
            // Look for // comments
            let mut desc_lines = Vec::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") {
                    desc_lines.push(trimmed.trim_start_matches('/').trim());
                } else if !trimmed.is_empty() {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        "shell" => {
            // Look for leading comments (skip shebang)
            let mut desc_lines = Vec::new();
            let mut skip_shebang = true;
            for line in content.lines() {
                let trimmed = line.trim();
                if skip_shebang && trimmed.starts_with("#!") {
                    skip_shebang = false;
                    continue;
                }
                if trimmed.starts_with('#') {
                    desc_lines.push(trimmed.trim_start_matches('#').trim());
                } else if !trimmed.is_empty() {
                    break;
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
            None
        }
        _ => None,
    }
}

/// Detect if script has a main entry point
fn detect_main_entry(content: &str, language: &str) -> bool {
    match language {
        "python" => content.contains("if __name__") && content.contains("__main__"),
        "node" | "typescript" => {
            content.contains("require.main === module") || 
            content.contains("import.meta.main") ||
            // Check for top-level execution patterns
            (!content.contains("module.exports") && !content.contains("export "))
        }
        "shell" => true, // Shell scripts are always executable
        _ => false,
    }
}

/// Detect if script uses argument parsing
fn detect_argparse_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("argparse") || 
            content.contains("sys.argv") || 
            content.contains("click") ||
            content.contains("typer")
        }
        "node" | "typescript" => {
            content.contains("process.argv") || 
            content.contains("yargs") || 
            content.contains("commander") ||
            content.contains("minimist")
        }
        "shell" => {
            content.contains("$1") || 
            content.contains("$@") || 
            content.contains("getopts") ||
            content.contains("${1")
        }
        _ => false,
    }
}

/// Detect if script uses stdin/stdout for I/O
fn detect_stdio_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("sys.stdin") || 
            content.contains("input()") || 
            content.contains("json.load(sys.stdin)") ||
            content.contains("print(") ||
            content.contains("json.dumps")
        }
        "node" | "typescript" => {
            content.contains("process.stdin") || 
            content.contains("readline") ||
            content.contains("console.log") ||
            content.contains("JSON.stringify")
        }
        "shell" => {
            content.contains("read ") || 
            content.contains("echo ") ||
            content.contains("cat ")
        }
        _ => false,
    }
}
