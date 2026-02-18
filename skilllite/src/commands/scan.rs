//! Skill scan command: scan directory and analyze executable scripts.

use crate::path_validation::validate_skill_path;
use crate::skill;
use anyhow::Result;
use std::fs;

/// Scan skill directory and return JSON with all executable scripts.
pub fn scan_skill(skill_dir: &str, preview_lines: usize) -> Result<String> {
    let skill_path = validate_skill_path(skill_dir)?;

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

    let skill_md_path = skill_path.join("SKILL.md");
    if skill_md_path.exists() {
        result["has_skill_md"] = serde_json::json!(true);
        if let Ok(metadata) = skill::metadata::parse_skill_metadata(&skill_path) {
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

    result["directories"]["scripts"] = serde_json::json!(skill_path.join("scripts").exists());
    result["directories"]["references"] = serde_json::json!(skill_path.join("references").exists());
    result["directories"]["assets"] = serde_json::json!(skill_path.join("assets").exists());

    let mut scripts = Vec::new();
    scan_scripts_recursive(&skill_path, &skill_path, &mut scripts, preview_lines)?;

    if let Some(entry_point) = result["skill_metadata"]["entry_point"].as_str() {
        for script in &mut scripts {
            if let Some(path) = script.get("path").and_then(|p| p.as_str()) {
                if path == entry_point {
                    script["is_entry_point"] = serde_json::json!(true);
                    script["confidence"] = serde_json::json!(1.0);
                    script["reasoning"] = serde_json::json!("Matches skill entry_point");
                } else {
                    script["is_entry_point"] = serde_json::json!(false);
                }
            }
        }
    }

    result["scripts"] = serde_json::json!(scripts);
    result["llm_prompt_hint"] = serde_json::json!(build_llm_prompt_hint(&result));

    serde_json::to_string_pretty(&result)
        .map_err(|e| anyhow::anyhow!("Failed to serialize scan result: {}", e))
}

fn build_llm_prompt_hint(result: &serde_json::Value) -> String {
    let mut hints = Vec::new();

    if let Some(meta) = result["skill_metadata"].as_object() {
        if let Some(desc) = meta.get("description").and_then(|v| v.as_str()) {
            hints.push(format!("Skill purpose: {}", desc));
        }
        if let Some(ep) = meta.get("entry_point").and_then(|v| v.as_str()) {
            hints.push(format!("Primary entry point: {}", ep));
        }
    }

    let scripts = match result["scripts"].as_array() {
        Some(s) => s,
        None => return hints.join("\n"),
    };
    if scripts.is_empty() {
        hints.push("No executable scripts found. This may be a prompt-only skill.".to_string());
    } else {
        let mut script_types: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for s in scripts {
            if let Some(lang) = s.get("language").and_then(|v| v.as_str()) {
                *script_types.entry(lang).or_insert(0) += 1;
            }
        }
        let type_str: Vec<String> = script_types
            .iter()
            .map(|(lang, count)| format!("{} {}", count, lang))
            .collect();
        hints.push(format!("Available scripts: {}", type_str.join(", ")));

        let described: Vec<_> = scripts
            .iter()
            .filter_map(|s| {
                let desc = s.get("description")?.as_str()?;
                let path = s.get("path")?.as_str()?;
                Some((path, desc))
            })
            .take(3)
            .collect();
        if !described.is_empty() {
            hints.push("Scripts with descriptions:".to_string());
            for (path, desc) in described {
                let truncated = if desc.len() > 100 {
                    format!("{}...", &desc[..100])
                } else {
                    desc.to_string()
                };
                hints.push(format!("  - {}: {}", path, truncated));
            }
        }
    }

    hints.join("\n")
}

fn scan_scripts_recursive(
    base_path: &std::path::Path,
    current_path: &std::path::Path,
    scripts: &mut Vec<serde_json::Value>,
    preview_lines: usize,
) -> Result<()> {
    let entries = fs::read_dir(current_path)
        .map_err(|e| anyhow::anyhow!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if file_name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let skip_dirs = ["node_modules", "__pycache__", ".git", "venv", ".venv", "assets", "references"];
            if skip_dirs.contains(&file_name.as_str()) {
                continue;
            }
            scan_scripts_recursive(base_path, &path, scripts, preview_lines)?;
            continue;
        }

        if let Some(script_info) = analyze_script_file(&path, base_path, preview_lines) {
            scripts.push(script_info);
        }
    }

    Ok(())
}

fn analyze_script_file(
    file_path: &std::path::Path,
    base_path: &std::path::Path,
    preview_lines: usize,
) -> Option<serde_json::Value> {
    let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let (language, is_script) = match extension {
        "py" => ("python", true),
        "js" | "mjs" | "cjs" => ("node", true),
        "ts" => ("typescript", true),
        "sh" | "bash" => ("shell", true),
        "" => {
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

    let preview: String = lines
        .iter()
        .take(preview_lines)
        .cloned()
        .collect::<Vec<&str>>()
        .join("\n");

    let description = extract_script_description(&content, language);
    let has_main = detect_main_entry(&content, language);
    let uses_argparse = detect_argparse_usage(&content, language);
    let uses_stdio = detect_stdio_usage(&content, language);

    let path_str = relative_path.to_string_lossy();
    let in_scripts_dir = path_str.starts_with("scripts/") || path_str.starts_with("scripts\\");

    let rec = compute_execution_recommendation_full(
        uses_stdio,
        uses_argparse,
        has_main,
        in_scripts_dir,
        false,
        None,
        &preview,
    );
    let suggested_command = generate_suggested_command(&path_str, language, rec.method);

    Some(serde_json::json!({
        "path": path_str,
        "language": language,
        "total_lines": total_lines,
        "preview": preview,
        "description": description,
        "has_main_entry": has_main,
        "uses_argparse": uses_argparse,
        "uses_stdio": uses_stdio,
        "in_scripts_dir": in_scripts_dir,
        "file_size_bytes": fs::metadata(file_path).map(|m| m.len()).unwrap_or(0),
        "execution_recommendation": rec.method,
        "confidence": rec.confidence,
        "reasoning": rec.reasoning,
        "suggested_command": suggested_command,
        "input_format": rec.input_format,
        "output_format": rec.output_format
    }))
}

fn extract_script_description(content: &str, language: &str) -> Option<String> {
    match language {
        "python" => {
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
            let trimmed = content.trim_start();
            if trimmed.starts_with("/**") {
                if let Some(end) = trimmed.find("*/") {
                    let doc = &trimmed[3..end];
                    let cleaned: Vec<&str> = doc
                        .lines()
                        .map(|l| l.trim().trim_start_matches('*').trim())
                        .filter(|l| !l.is_empty())
                        .collect();
                    return Some(cleaned.join(" "));
                }
            }
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

fn detect_main_entry(content: &str, language: &str) -> bool {
    match language {
        "python" => content.contains("if __name__") && content.contains("__main__"),
        "node" | "typescript" => {
            content.contains("require.main === module")
                || content.contains("import.meta.main")
                || (!content.contains("module.exports") && !content.contains("export "))
        }
        "shell" => true,
        _ => false,
    }
}

fn detect_argparse_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("argparse")
                || content.contains("sys.argv")
                || content.contains("click")
                || content.contains("typer")
        }
        "node" | "typescript" => {
            content.contains("process.argv")
                || content.contains("yargs")
                || content.contains("commander")
                || content.contains("minimist")
        }
        "shell" => {
            content.contains("$1")
                || content.contains("$@")
                || content.contains("getopts")
                || content.contains("${1")
        }
        _ => false,
    }
}

fn detect_stdio_usage(content: &str, language: &str) -> bool {
    match language {
        "python" => {
            content.contains("sys.stdin")
                || content.contains("input()")
                || content.contains("json.load(sys.stdin)")
                || content.contains("print(")
                || content.contains("json.dumps")
        }
        "node" | "typescript" => {
            content.contains("process.stdin")
                || content.contains("readline")
                || content.contains("console.log")
                || content.contains("JSON.stringify")
        }
        "shell" => content.contains("read ") || content.contains("echo ") || content.contains("cat "),
        _ => false,
    }
}

struct ExecutionRecommendation {
    method: &'static str,
    confidence: f64,
    reasoning: String,
    input_format: &'static str,
    output_format: &'static str,
}

fn compute_execution_recommendation_full(
    uses_stdio: bool,
    uses_argparse: bool,
    has_main: bool,
    in_scripts_dir: bool,
    is_entry_point: bool,
    entry_point_reasoning: Option<&'static str>,
    preview: &str,
) -> ExecutionRecommendation {
    let mut reasoning_parts: Vec<&'static str> = Vec::new();

    if let Some(r) = entry_point_reasoning {
        reasoning_parts.insert(0, r);
    }

    let (method, confidence, input_format, output_format) = if is_entry_point {
        let m = if uses_stdio && !uses_argparse {
            "stdin_json"
        } else if uses_argparse {
            "argparse"
        } else {
            "direct"
        };
        if reasoning_parts.is_empty() {
            reasoning_parts.push("Matches skill entry_point");
        }
        let (in_fmt, out_fmt) = format_for_method(m, uses_stdio, preview);
        (m, 1.0, in_fmt, out_fmt)
    } else {
        let mut conf: f64 = 0.0;
        if uses_stdio {
            conf += 0.3;
            reasoning_parts.push("Script uses stdin/stdout for I/O");
        }
        if uses_argparse {
            conf += 0.2;
            reasoning_parts.push("Script uses argument parsing");
        }
        if has_main {
            conf += 0.1;
            reasoning_parts.push("Has main entry point");
        }
        if in_scripts_dir {
            conf += 0.1;
            reasoning_parts.push("Located in scripts/ directory");
        }
        if !uses_stdio && !uses_argparse {
            reasoning_parts.push("Script appears to run directly without input");
        }

        let m = if uses_stdio && !uses_argparse {
            "stdin_json"
        } else if uses_argparse {
            "argparse"
        } else {
            "direct"
        };
        let (in_fmt, out_fmt) = format_for_method(m, uses_stdio, preview);
        (m, conf.min(1.0), in_fmt, out_fmt)
    };

    let reasoning = reasoning_parts.join("; ");
    ExecutionRecommendation {
        method,
        confidence: (confidence * 100.0).round() / 100.0,
        reasoning,
        input_format,
        output_format,
    }
}

fn format_for_method(method: &str, uses_stdio: bool, preview: &str) -> (&'static str, &'static str) {
    let input_format = match method {
        "stdin_json" => "json_stdin",
        "argparse" => "cli_args",
        _ => "none",
    };
    let output_format = if uses_stdio {
        if preview.to_lowercase().contains("json") {
            "json_stdout"
        } else {
            "text_stdout"
        }
    } else {
        "text_stdout"
    };
    (input_format, output_format)
}

fn generate_suggested_command(path: &str, language: &str, method: &str) -> String {
    match method {
        "stdin_json" => match language {
            "python" => format!("echo '{{\"input\": \"value\"}}' | python {path}"),
            "node" | "typescript" => format!("echo '{{\"input\": \"value\"}}' | node {path}"),
            "shell" => format!("echo '{{\"input\": \"value\"}}' | bash {path}"),
            _ => format!("# Unknown execution method for {path}"),
        },
        "argparse" => match language {
            "python" => format!("python {path} --help"),
            "node" | "typescript" => format!("node {path} --help"),
            "shell" => format!("bash {path} --help"),
            _ => format!("# Unknown execution method for {path}"),
        },
        _ => match language {
            "python" => format!("python {path}"),
            "node" | "typescript" => format!("node {path}"),
            "shell" => format!("bash {path}"),
            _ => format!("# Unknown execution method for {path}"),
        },
    }
}
