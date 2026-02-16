//! Prompt construction for the agent.
//!
//! Ported from Python `PromptBuilder`. Generates system prompt context
//! from loaded skills, including progressive disclosure support.

use super::skills::LoadedSkill;
use super::types::get_output_dir;

/// Default system prompt for the agent.
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a helpful AI assistant with access to tools.

CRITICAL RULE — you MUST actually call tools to perform actions. NEVER claim you have completed a task (e.g. "访问了百度", "截图保存为...", "完成！") unless you have ACTUALLY invoked the corresponding tool in this turn and received a successful result. If a task requires using a skill or tool, you MUST call it — do NOT skip the tool call and fabricate a completion message.

When using tools:
- Use read_file to read file contents before modifying them
- Use write_file to create or update files
- Use write_output to write final text deliverables to the output directory
- Use list_directory to explore the workspace structure
- Use file_exists to check if files/directories exist before operations
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

    // Skills context (progressive disclosure: summary only, full docs on first call)
    if !skills.is_empty() {
        parts.push("\n\n## Available Skills\n".to_string());
        for skill in skills {
            let desc = skill.metadata.description.as_deref().unwrap_or("No description");
            let entry = if skill.metadata.entry_point.is_empty() {
                "(prompt-only)"
            } else if skill.metadata.is_bash_tool_skill() {
                "(bash-tool)"
            } else {
                ""
            };
            parts.push(format!("- **{}** {}: {}", skill.name, entry, desc));
        }
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
