//! Prompt construction for the agent.
//!
//! Ported from Python `PromptBuilder`. Generates system prompt context
//! from loaded skills, including progressive disclosure support.
//!
//! ## Progressive Disclosure Modes
//!
//! Four prompt modes control how much skill information is included:
//!
//! | Mode        | Content                                       | Usage           |
//! |-------------|-----------------------------------------------|-----------------|
//! | Summary     | Skill name + 150-char description              | Compact views   |
//! | Standard    | Schema + 200-char description                 | Default prompts |
//! | Progressive | Standard + "more details available" hint       | Agent system    |
//! | Full        | Complete SKILL.md + references + assets        | First invocation|

use super::skills::LoadedSkill;
use super::types::{get_output_dir, safe_truncate};

/// Progressive disclosure mode.
/// Summary/Standard/Full are used in tests and for API completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PromptMode {
    /// 150-char description only.
    Summary,
    /// Schema + 200-char description.
    Standard,
    /// Standard + "use get_skill_info for full docs" hint.
    Progressive,
    /// Complete SKILL.md + reference files.
    Full,
}

/// Default system prompt for the agent.
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to tools.

CRITICAL RULE — you MUST actually call tools to perform actions. NEVER claim you have completed a task (e.g. "访问了百度", "截图保存为...", "完成！") unless you have ACTUALLY invoked the corresponding tool in this turn and received a successful result. If a task requires using a skill or tool, you MUST call it — do NOT skip the tool call and fabricate a completion message.

When using tools:
- Use read_file to read file contents before modifying them
- Use write_file to create or update files
- Use write_output to write final text deliverables to the output directory
- Use list_directory to explore the workspace structure
- Use file_exists to check if files/directories exist before operations
- Use chat_history to read past conversation when the user asks to view, summarize, or analyze chat records (supports date filter)
- Use chat_plan to read task plans when the user asks about today's plan or task status
- Use list_output to list files in the output directory (no path needed)
- Use run_command to execute shell commands (requires user confirmation)
- Always work within the workspace directory

When executing skills:
- Skills are sandboxed tools that run in isolation
- Pass the required input parameters as specified in the skill description
- Review skill output carefully before proceeding

Be concise and accurate. Focus on completing the user's request efficiently."#;

/// Build the complete system prompt.
pub fn build_system_prompt(
    custom_prompt: Option<&str>,
    skills: &[LoadedSkill],
    workspace: &str,
) -> String {
    let mut parts = Vec::new();

    // Base system prompt
    parts.push(custom_prompt.unwrap_or(DEFAULT_SYSTEM_PROMPT).to_string());

    // Workspace context
    parts.push(format!("\n\nWorkspace: {}", workspace));

    // Output directory context — LLM must use $SKILLLITE_OUTPUT_DIR for file outputs
    let output_dir = get_output_dir().unwrap_or_else(|| format!("{}/output", workspace));
    parts.push(format!("\nOutput directory: {}", output_dir));
    parts.push(format!(
        concat!(
            "\n\nIMPORTANT — File output path rule for bash-tool skills:\n",
            "Some CLI tools (e.g. agent-browser) do NOT save files to the shell's working directory.\n",
            "You MUST always use the absolute output path via the $SKILLLITE_OUTPUT_DIR environment variable.\n",
            "Example: `agent-browser screenshot $SKILLLITE_OUTPUT_DIR/screenshot.png`\n",
            "The shell will expand $SKILLLITE_OUTPUT_DIR to: {}\n",
            "NEVER use bare filenames like `screenshot.png` — always prefix with $SKILLLITE_OUTPUT_DIR/",
        ),
        output_dir
    ));

    // Skills context — Progressive mode: summary + "more details available" hint.
    // Full docs are injected on first tool call via inject_progressive_disclosure.
    if !skills.is_empty() {
        parts.push(build_skills_context(skills, PromptMode::Progressive));
    }

    // Bash-tool skills: inject full SKILL.md content upfront
    // (same as Python _get_bash_tool_skills_context)
    let bash_skills: Vec<_> = skills.iter().filter(|s| s.metadata.is_bash_tool_skill()).collect();
    if !bash_skills.is_empty() {
        parts.push("\n\n## Bash-Tool Skills Documentation\n".to_string());
        for skill in bash_skills {
            let skill_md_path = skill.skill_dir.join("SKILL.md");
            if let Ok(content) = std::fs::read_to_string(&skill_md_path) {
                parts.push(format!(
                    "### {}\n\n{}\n",
                    skill.name,
                    content
                ));
            }
        }
    }

    parts.join("")
}

/// Build skills context section for the system prompt.
///
/// Uses the specified `PromptMode` to control verbosity:
///   - Summary: name + 150-char truncated description
///   - Standard: name + 200-char description + parameter schema hints
///   - Progressive: Standard + "more details available" hint
///   - Full: complete SKILL.md content (rarely used in system prompt)
pub fn build_skills_context(skills: &[LoadedSkill], mode: PromptMode) -> String {
    let mut parts = vec!["\n\n## Available Skills\n".to_string()];

    for skill in skills {
        let raw_desc = skill
            .metadata
            .description
            .as_deref()
            .unwrap_or("No description");
        let entry_tag = if skill.metadata.entry_point.is_empty() {
            if skill.metadata.is_bash_tool_skill() {
                " (bash-tool)"
            } else {
                " (prompt-only)"
            }
        } else {
            ""
        };

        match mode {
            PromptMode::Summary => {
                let truncated = safe_truncate(raw_desc, 150);
                parts.push(format!("- **{}**{}: {}", skill.name, entry_tag, truncated));
            }
            PromptMode::Standard => {
                let truncated = safe_truncate(raw_desc, 200);
                let schema_hint = build_schema_hint(skill);
                parts.push(format!(
                    "- **{}**{}: {}{}",
                    skill.name, entry_tag, truncated, schema_hint
                ));
            }
            PromptMode::Progressive => {
                let truncated = safe_truncate(raw_desc, 200);
                let schema_hint = build_schema_hint(skill);
                parts.push(format!(
                    "- **{}**{}: {}{}",
                    skill.name, entry_tag, truncated, schema_hint
                ));
            }
            PromptMode::Full => {
                if let Some(docs) = get_skill_full_docs(skill) {
                    parts.push(format!("### {}\n\n{}", skill.name, docs));
                } else {
                    parts.push(format!("- **{}**{}: {}", skill.name, entry_tag, raw_desc));
                }
            }
        }
    }

    if mode == PromptMode::Progressive {
        parts.push(
            "\n> Tip: Full documentation for each skill will be provided when you first call it."
                .to_string(),
        );
    }

    parts.join("\n")
}

/// Build a brief schema hint showing required parameters.
fn build_schema_hint(skill: &LoadedSkill) -> String {
    if let Some(first_tool) = skill.tool_definitions.first() {
        if let Some(required) = first_tool.function.parameters.get("required") {
            if let Some(arr) = required.as_array() {
                let params: Vec<&str> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect();
                if !params.is_empty() {
                    return format!(" (params: {})", params.join(", "));
                }
            }
        }
    }
    String::new()
}

/// Get full skill documentation for progressive disclosure.
/// Called when the LLM first invokes a skill tool.
/// Returns the SKILL.md content plus reference docs.
pub fn get_skill_full_docs(skill: &LoadedSkill) -> Option<String> {
    let skill_md_path = skill.skill_dir.join("SKILL.md");
    let mut parts = Vec::new();

    if let Ok(content) = std::fs::read_to_string(&skill_md_path) {
        parts.push(format!(
            "## Full Documentation for skill: {}\n\n{}",
            skill.name, content
        ));
    } else {
        return None;
    }

    // Include reference files if present
    let refs_dir = skill.skill_dir.join("references");
    if refs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&refs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let name = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        // Limit reference content
                        let truncated = if content.len() > 5000 {
                            format!("{}...\n[truncated]", &content[..5000])
                        } else {
                            content
                        };
                        parts.push(format!(
                            "\n### Reference: {}\n\n{}",
                            name, truncated
                        ));
                    }
                }
            }
        }
    }

    Some(parts.join(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::metadata::SkillMetadata;
    use std::collections::HashMap;

    fn make_test_skill(name: &str, desc: &str) -> LoadedSkill {
        use super::super::types::{ToolDefinition, FunctionDef};
        LoadedSkill {
            name: name.to_string(),
            skill_dir: std::path::PathBuf::from("/tmp/test-skill"),
            metadata: SkillMetadata {
                name: name.to_string(),
                entry_point: "scripts/main.py".to_string(),
                language: Some("python".to_string()),
                description: Some(desc.to_string()),
                compatibility: None,
                network: Default::default(),
                resolved_packages: None,
                allowed_tools: None,
                requires_elevated_permissions: false,
            },
            tool_definitions: vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: name.to_string(),
                    description: desc.to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "input": {"type": "string", "description": "Input text"}
                        },
                        "required": ["input"]
                    }),
                },
            }],
            multi_script_entries: HashMap::new(),
        }
    }

    #[test]
    fn test_prompt_mode_summary() {
        let skills = vec![make_test_skill("calculator", "A very useful calculator skill for mathematical operations and complex computations that can handle everything")];
        let ctx = build_skills_context(&skills, PromptMode::Summary);

        assert!(ctx.contains("calculator"));
        assert!(ctx.contains("Available Skills"));
        // Summary mode truncates to 150 chars
        assert!(!ctx.contains("Tip:")); // No progressive hint
    }

    #[test]
    fn test_prompt_mode_standard() {
        let skills = vec![make_test_skill("calculator", "Does math")];
        let ctx = build_skills_context(&skills, PromptMode::Standard);

        assert!(ctx.contains("calculator"));
        assert!(ctx.contains("Does math"));
        assert!(ctx.contains("(params: input)")); // Schema hint
        assert!(!ctx.contains("Tip:")); // No progressive hint
    }

    #[test]
    fn test_prompt_mode_progressive() {
        let skills = vec![make_test_skill("calculator", "Does math")];
        let ctx = build_skills_context(&skills, PromptMode::Progressive);

        assert!(ctx.contains("calculator"));
        assert!(ctx.contains("Does math"));
        assert!(ctx.contains("(params: input)"));
        assert!(ctx.contains("Tip:")); // Has progressive hint
        assert!(ctx.contains("Full documentation"));
    }

    #[test]
    fn test_build_system_prompt_contains_workspace() {
        let prompt = build_system_prompt(None, &[], "/home/user/project");
        assert!(prompt.contains("Workspace: /home/user/project"));
    }

    #[test]
    fn test_build_system_prompt_uses_progressive_mode() {
        let skills = vec![make_test_skill("test-skill", "Test description")];
        let prompt = build_system_prompt(None, &skills, "/tmp");

        assert!(prompt.contains("test-skill"));
        assert!(prompt.contains("Test description"));
        assert!(prompt.contains("Tip:")); // Progressive mode hint
    }

    #[test]
    fn test_build_schema_hint() {
        let skill = make_test_skill("test", "desc");
        let hint = build_schema_hint(&skill);
        assert_eq!(hint, " (params: input)");
    }

    #[test]
    fn test_build_schema_hint_no_required() {
        use super::super::types::{ToolDefinition, FunctionDef};
        let skill = LoadedSkill {
            name: "test".to_string(),
            skill_dir: std::path::PathBuf::from("/tmp"),
            metadata: SkillMetadata {
                name: "test".to_string(),
                entry_point: String::new(),
                language: None,
                description: None,
                compatibility: None,
                network: Default::default(),
                resolved_packages: None,
                allowed_tools: None,
                requires_elevated_permissions: false,
            },
            tool_definitions: vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: "test".to_string(),
                    description: "test".to_string(),
                    parameters: serde_json::json!({"type": "object", "properties": {}}),
                },
            }],
            multi_script_entries: HashMap::new(),
        };
        let hint = build_schema_hint(&skill);
        assert_eq!(hint, "");
    }
}
