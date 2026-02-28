//! Skill loading: discovers skill directories, parses SKILL.md, generates tool definitions.

use std::collections::HashMap;
use std::path::Path;

use skilllite_core::skill::metadata::{self, SkillMetadata};

use crate::types::{ToolDefinition, FunctionDef};

use super::LoadedSkill;

pub(super) fn load_evolved_skills(evolved_dir: &Path) -> Vec<LoadedSkill> {
    let mut skills = Vec::new();

    let entries = match std::fs::read_dir(evolved_dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() || !skill_dir.join("SKILL.md").exists() {
            continue;
        }

        // Check .meta.json for archived status
        let meta_path = skill_dir.join(".meta.json");
        if meta_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<crate::evolution::skill_synth::SkillMeta>(&content) {
                    if meta.archived {
                        tracing::debug!("Skipping archived evolved skill: {}", meta.name);
                        continue;
                    }
                }
            }
        }

        if let Some(skill) = load_single_skill(&skill_dir) {
            tracing::debug!("Loaded evolved skill: {}", skill.name);
            skills.push(skill);
        }
    }

    skills
}

/// Load a single skill from a directory.

pub(super) fn load_single_skill(skill_dir: &Path) -> Option<LoadedSkill> {
    let metadata = match metadata::parse_skill_metadata(skill_dir) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to parse skill at {}: {}", skill_dir.display(), e);
            return None;
        }
    };

    let name = metadata.name.clone();
    let tool_name = sanitize_tool_name(&name);

    // Generate tool definition based on skill type
    let mut tool_defs = if metadata.is_bash_tool_skill() {
        // Bash-tool skill: command string parameter
        let patterns = metadata.get_bash_patterns();
        let desc = metadata.description.clone().unwrap_or_else(|| {
            format!("Execute commands for {}. Allowed patterns: {:?}", name, patterns)
        });
        vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: tool_name,
                description: desc,
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": format!("Bash command to execute. Must match allowed patterns: {:?}", patterns)
                        }
                    },
                    "required": ["command"]
                }),
            },
        }]
    } else if !metadata.entry_point.is_empty() {
        // Regular skill with entry point — try argparse schema inference
        let desc = metadata.description.clone().unwrap_or_else(|| {
            format!("Execute skill: {}", name)
        });
        let schema = infer_entry_point_schema(skill_dir, &metadata)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": true
                })
            });
        vec![ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: tool_name,
                description: desc,
                parameters: schema,
            },
        }]
    } else {
        // No single entry point — check for multi-script skill
        Vec::new()
    };

    // Phase 2.5: detect multi-script skills (scripts/ directory with multiple entry points)
    let mut multi_script_entries = HashMap::new();
    if tool_defs.is_empty() && !metadata.is_bash_tool_skill() {
        let (multi_tools, entries) = detect_multi_script_tools(skill_dir, &name);
        tool_defs.extend(multi_tools);
        multi_script_entries = entries;
    }

    Some(LoadedSkill {
        name,
        skill_dir: skill_dir.to_path_buf(),
        metadata,
        tool_definitions: tool_defs,
        multi_script_entries,
    })
}

// ─── Phase 2.5: Multi-script skill support ──────────────────────────────────

/// Detect multiple scripts in a skill's `scripts/` directory and generate
/// a separate tool definition for each.
/// Returns (tool_definitions, entry_map: tool_name → script_path).
/// Ported from Python `detect_all_scripts` + `analyze_multi_script_skill`.

fn detect_multi_script_tools(
    skill_dir: &Path,
    skill_name: &str,
) -> (Vec<ToolDefinition>, HashMap<String, String>) {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() || !scripts_dir.is_dir() {
        return (Vec::new(), HashMap::new());
    }

    let extensions = [
        (".py", "python"),
        (".js", "node"),
        (".ts", "node"),
        (".sh", "bash"),
    ];

    let skip_names = ["__init__.py"];
    let mut tools = Vec::new();
    let mut entries = HashMap::new();

    for (ext, _lang) in &extensions {
        if let Ok(dir_entries) = std::fs::read_dir(&scripts_dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                let fname = path.file_name().map(|n| n.to_string_lossy().to_string());
                let fname = match fname {
                    Some(f) => f,
                    None => continue,
                };

                if !fname.ends_with(ext) {
                    continue;
                }
                if fname.starts_with("test_")
                    || fname.ends_with("_test.py")
                    || fname.starts_with('.')
                    || skip_names.contains(&fname.as_str())
                {
                    continue;
                }

                let script_stem = fname.trim_end_matches(ext).replace('_', "-");
                // Tool name: skill_name__script_name (double underscore)
                let tool_name = format!(
                    "{}__{}",
                    sanitize_tool_name(skill_name),
                    sanitize_tool_name(&script_stem)
                );

                let script_path = format!("scripts/{}", fname);

                let desc = format!(
                    "Execute {} from skill '{}'",
                    script_path, skill_name
                );

                // Try argparse inference for Python scripts
                let schema = if fname.ends_with(".py") {
                    parse_argparse_schema(&path).unwrap_or_else(|| flexible_schema())
                } else {
                    flexible_schema()
                };

                // Store the mapping: tool_name → script_path
                entries.insert(tool_name.clone(), script_path);

                tools.push(ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDef {
                        name: tool_name,
                        description: desc,
                        parameters: schema,
                    },
                });
            }
        }
    }

    (tools, entries)
}

/// Return a flexible JSON schema that accepts any properties.

fn flexible_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

// ─── Phase 2.5: Argparse schema inference ───────────────────────────────────

/// Try to infer parameter schema from a skill's entry point script.
/// If the entry point is a Python file, parse argparse calls.

fn infer_entry_point_schema(skill_dir: &Path, metadata: &SkillMetadata) -> Option<serde_json::Value> {
    let entry = &metadata.entry_point;
    if entry.is_empty() {
        return None;
    }
    let script_path = skill_dir.join(entry);
    if script_path.extension().and_then(|e| e.to_str()) != Some("py") {
        return None;
    }
    parse_argparse_schema(&script_path)
}

/// Parse Python script for argparse `add_argument` calls and generate JSON schema.
/// Ported from Python `tool_builder.py` `_parse_argparse_schema`.

fn parse_argparse_schema(script_path: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(script_path).ok()?;

    let arg_re = regex::Regex::new(
        r#"\.add_argument\s*\(\s*['"]([^'"]+)['"](?:\s*,\s*['"]([^'"]+)['"])?([^)]*)\)"#,
    )
    .ok()?;

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for caps in arg_re.captures_iter(&content) {
        let arg_name = caps.get(1)?.as_str();
        let second_arg = caps.get(2).map(|m| m.as_str());
        let kwargs_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        // Determine parameter name
        let (param_name, is_positional) = if arg_name.starts_with("--") {
            (arg_name[2..].replace('-', "_"), false)
        } else if arg_name.starts_with('-') {
            if let Some(s) = second_arg {
                if s.starts_with("--") {
                    (s[2..].replace('-', "_"), false)
                } else {
                    (arg_name[1..].to_string(), false)
                }
            } else {
                (arg_name[1..].to_string(), false)
            }
        } else {
            (arg_name.replace('-', "_"), true)
        };

        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), serde_json::json!("string"));

        // Extract help text
        if let Some(help_cap) = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            if let Some(m) = help_cap.get(1) {
                prop.insert(
                    "description".to_string(),
                    serde_json::json!(m.as_str()),
                );
            }
        }

        // Extract type
        if let Some(type_cap) = regex::Regex::new(r"type\s*=\s*(\w+)")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            match type_cap.get(1).map(|m| m.as_str()).unwrap_or("") {
                "int" => {
                    prop.insert("type".to_string(), serde_json::json!("integer"));
                }
                "float" => {
                    prop.insert("type".to_string(), serde_json::json!("number"));
                }
                "bool" => {
                    prop.insert("type".to_string(), serde_json::json!("boolean"));
                }
                _ => {}
            }
        }

        // Check action=store_true/store_false
        if let Some(action_cap) = regex::Regex::new(r#"action\s*=\s*['"](\w+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let action = action_cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if action == "store_true" || action == "store_false" {
                prop.insert("type".to_string(), serde_json::json!("boolean"));
            }
        }

        // Check nargs
        if let Some(nargs_cap) = regex::Regex::new(r#"nargs\s*=\s*['"]?([^,\s)]+)['"]?"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let nargs = nargs_cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if nargs == "*" || nargs == "+" || nargs.parse::<u32>().is_ok() {
                prop.insert("type".to_string(), serde_json::json!("array"));
                prop.insert("items".to_string(), serde_json::json!({"type": "string"}));
            }
        }

        // Check choices
        if let Some(choices_cap) = regex::Regex::new(r"choices\s*=\s*\[([^\]]+)\]")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let choices_str = choices_cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let choices: Vec<String> = regex::Regex::new(r#"['"]([^'"]+)['"]"#)
                .ok()
                .map(|re| {
                    re.captures_iter(choices_str)
                        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if !choices.is_empty() {
                prop.insert("enum".to_string(), serde_json::json!(choices));
            }
        }

        // Check default
        if let Some(default_cap) = regex::Regex::new(r"default\s*=\s*([^,)]+)")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let val = default_cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            if val != "None" && val != "\"\"" && val != "''" {
                let cleaned = val.trim_matches(|c| c == '"' || c == '\'');
                prop.insert("default".to_string(), serde_json::json!(cleaned));
            }
        }

        // Check required
        let is_required = kwargs_str.contains("required=True") || is_positional;
        if is_required {
            required.push(param_name.clone());
        }

        properties.insert(param_name, serde_json::Value::Object(prop));
    }

    if properties.is_empty() {
        return None;
    }

    Some(serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required
    }))
}

/// Sanitize skill name to a valid tool function name.
/// Replaces non-alphanumeric chars with underscore, lowercases.

pub(super) fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}
