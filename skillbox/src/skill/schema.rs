//! Schema inference for skill list --json output.
//!
//! Provides multi-script tool detection and argparse schema parsing
//! for Python SDK delegation (Phase 4.8 Metadata 委托).

use serde_json::{json, Value};
use std::path::Path;

/// Multi-script tool entry for list --json output.
#[derive(Debug)]
pub struct MultiScriptTool {
    pub tool_name: String,
    pub skill_name: String,
    pub script_path: String,
    pub input_schema: Value,
    pub description: String,
}

/// Detect all executable scripts in a skill directory and return tool definitions.
/// Used when skill has no entry_point (multi-script skill like skill-creator).
pub fn detect_multi_script_tools(skill_dir: &Path, skill_name: &str) -> Vec<MultiScriptTool> {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() || !scripts_dir.is_dir() {
        return Vec::new();
    }

    let extensions = [(".py", "python"), (".js", "node"), (".ts", "node"), (".sh", "bash")];
    let skip_names = ["__init__.py"];
    let mut tools = Vec::new();

    for (ext, _lang) in &extensions {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let fname = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

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
                let tool_name = format!(
                    "{}__{}",
                    sanitize_tool_name(skill_name),
                    sanitize_tool_name(&script_stem)
                );
                let script_path = format!("scripts/{}", fname);

                let desc = format!("Execute {} from {} skill", script_path, skill_name);

                let input_schema = if fname.ends_with(".py") {
                    parse_argparse_schema(&path).unwrap_or_else(flexible_schema)
                } else {
                    flexible_schema()
                };

                tools.push(MultiScriptTool {
                    tool_name,
                    skill_name: skill_name.to_string(),
                    script_path,
                    input_schema,
                    description: desc,
                });
            }
        }
    }

    tools
}

/// Parse Python script for argparse `add_argument` calls and generate JSON schema.
pub fn parse_argparse_schema(script_path: &Path) -> Option<Value> {
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
        prop.insert("type".to_string(), json!("string"));

        if let Some(help_cap) = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            prop.insert(
                "description".to_string(),
                json!(help_cap.get(1).unwrap().as_str()),
            );
        }

        if let Some(type_cap) = regex::Regex::new(r"type\s*=\s*(\w+)")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            match type_cap.get(1).unwrap().as_str() {
                "int" => {
                    let _ = prop.insert("type".to_string(), json!("integer"));
                }
                "float" => {
                    let _ = prop.insert("type".to_string(), json!("number"));
                }
                "bool" => {
                    let _ = prop.insert("type".to_string(), json!("boolean"));
                }
                _ => {}
            };
        }

        if let Some(action_cap) = regex::Regex::new(r#"action\s*=\s*['"](\w+)['"]"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let action = action_cap.get(1).unwrap().as_str();
            if action == "store_true" || action == "store_false" {
                prop.insert("type".to_string(), json!("boolean"));
            }
        }

        if let Some(nargs_cap) = regex::Regex::new(r#"nargs\s*=\s*['"]?([^,\s)]+)['"]?"#)
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let nargs = nargs_cap.get(1).unwrap().as_str();
            if nargs == "*" || nargs == "+" || nargs.parse::<u32>().is_ok() {
                prop.insert("type".to_string(), json!("array"));
                prop.insert("items".to_string(), json!({"type": "string"}));
            }
        }

        if let Some(choices_cap) = regex::Regex::new(r"choices\s*=\s*\[([^\]]+)\]")
            .ok()
            .and_then(|re| re.captures(kwargs_str))
        {
            let choices_str = choices_cap.get(1).unwrap().as_str();
            let choices: Vec<String> = regex::Regex::new(r#"['"]([^'"]+)['"]"#)
                .ok()
                .map(|re| {
                    re.captures_iter(choices_str)
                        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if !choices.is_empty() {
                prop.insert("enum".to_string(), json!(choices));
            }
        }

        let is_required = kwargs_str.contains("required=True") || is_positional;
        if is_required {
            required.push(param_name.clone());
        }

        properties.insert(param_name, Value::Object(prop));
    }

    if properties.is_empty() {
        return None;
    }

    Some(json!({
        "type": "object",
        "properties": properties,
        "required": required
    }))
}

fn flexible_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

fn sanitize_tool_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}
