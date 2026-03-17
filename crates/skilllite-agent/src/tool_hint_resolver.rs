//! ToolHintResolver: single source of truth for tool_hint → tool name mapping.
//!
//! All hint resolution, availability checks, guidance text, and prompt generation
//! go through this module. Adding a new builtin hint means adding one entry to
//! `BUILTIN_HINTS`; everything else (prompt rules, guidance, preferred tools)
//! derives from it automatically.

use crate::extensions::ToolAvailabilityView;
use crate::skills::LoadedSkill;

/// A single builtin hint registration.
struct BuiltinHint {
    /// The hint name used in task plans (e.g. "file_read").
    hint: &'static str,
    /// Actual tool names this hint maps to. Empty means "no tool call needed" (e.g. analysis).
    tools: &'static [&'static str],
    /// Human-readable guidance injected into prompts. `None` for analysis-type hints.
    guidance: Option<&'static str>,
}

/// Central registry of builtin hints. This is the **only** place where
/// hint-to-tool mappings are defined.
const BUILTIN_HINTS: &[BuiltinHint] = &[
    BuiltinHint {
        hint: "file_list",
        tools: &["list_directory", "file_exists"],
        guidance: Some("Preferred tools: `list_directory` (and `file_exists` if needed)."),
    },
    BuiltinHint {
        hint: "file_read",
        tools: &["read_file", "file_exists"],
        guidance: Some("Preferred tools: `read_file` (and `file_exists` if needed)."),
    },
    BuiltinHint {
        hint: "file_write",
        tools: &["write_output", "write_file"],
        guidance: Some("Preferred tools: `write_output` or `write_file`. Generate the content yourself unless the task explicitly needs another tool."),
    },
    BuiltinHint {
        hint: "file_edit",
        tools: &["read_file", "file_exists", "search_replace", "preview_edit", "write_file"],
        guidance: Some("Preferred tools: `read_file`, `search_replace`, `preview_edit`, or `write_file` for targeted edits."),
    },
    BuiltinHint {
        hint: "file_operation",
        tools: &["read_file", "list_directory", "file_exists", "write_output", "write_file", "search_replace", "preview_edit", "preview_server", "run_command"],
        guidance: Some("Legacy broad file task: prefer built-in file tools. If the plan no longer fits, revise it with `update_task_plan`."),
    },
    BuiltinHint {
        hint: "preview",
        tools: &["preview_server"],
        guidance: Some("Preferred tool: `preview_server`."),
    },
    BuiltinHint {
        hint: "command",
        tools: &["run_command"],
        guidance: Some("Preferred tool: `run_command`."),
    },
    BuiltinHint {
        hint: "chat_history",
        tools: &["chat_history"],
        guidance: Some("Preferred tool: `chat_history`."),
    },
    BuiltinHint {
        hint: "memory_write",
        tools: &["memory_write"],
        guidance: Some("Preferred tool: `memory_write`."),
    },
    BuiltinHint {
        hint: "memory_search",
        tools: &["memory_search", "memory_list"],
        guidance: Some("Preferred tools: `memory_search` (or `memory_list` if needed)."),
    },
    BuiltinHint {
        hint: "analysis",
        tools: &[],
        guidance: None,
    },
];

fn normalize_hint_name(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

fn find_builtin(hint: &str) -> Option<&'static BuiltinHint> {
    BUILTIN_HINTS.iter().find(|b| b.hint == hint)
}

fn available_builtin_tools(hint: &str, availability: &ToolAvailabilityView) -> Vec<String> {
    find_builtin(hint)
        .map(|builtin| {
            let mut tools: Vec<String> = builtin
                .tools
                .iter()
                .filter(|tool| availability.has_tool(tool))
                .map(|tool| (*tool).to_string())
                .collect();
            tools.sort();
            tools.dedup();
            tools
        })
        .unwrap_or_default()
}

/// Check if a hint is a known builtin hint.
pub fn is_builtin_hint(hint: &str) -> bool {
    find_builtin(hint).is_some()
}

/// All known builtin hint names (for filtering rules by availability).
pub fn builtin_hint_names() -> Vec<&'static str> {
    BUILTIN_HINTS.iter().map(|b| b.hint).collect()
}

/// Resolve a hint to its preferred tool names (sorted, deduplicated).
/// For skill hints (not builtin), returns the normalized hint name itself.
pub fn preferred_tool_names(hint: &str) -> Vec<String> {
    if let Some(builtin) = find_builtin(hint) {
        let mut tools: Vec<String> = builtin.tools.iter().map(|s| s.to_string()).collect();
        tools.sort();
        tools.dedup();
        tools
    } else if hint.is_empty() {
        Vec::new()
    } else {
        vec![normalize_hint_name(hint)]
    }
}

/// Resolve a hint using the final availability view from the registry.
pub fn preferred_tool_names_with_availability(
    hint: &str,
    availability: &ToolAvailabilityView,
) -> Vec<String> {
    if let Some(builtin) = find_builtin(hint) {
        if builtin.tools.is_empty() {
            return Vec::new();
        }
        return available_builtin_tools(hint, availability);
    }

    if hint.is_empty() {
        return Vec::new();
    }

    let normalized = normalize_hint_name(hint);
    if availability.has_tool(&normalized) || availability.has_skill_hint(hint) {
        vec![normalized]
    } else {
        Vec::new()
    }
}

/// Get human-readable guidance for a hint. Returns `None` for unknown or analysis hints.
pub fn hint_guidance(hint: &str) -> Option<&'static str> {
    find_builtin(hint).and_then(|b| b.guidance)
}

/// Build guidance from the final availability view. For builtin hints this keeps
/// the old guidance style, but only for the tools that are truly callable now.
pub fn hint_guidance_with_availability(
    hint: &str,
    availability: &ToolAvailabilityView,
) -> Option<String> {
    if hint == "analysis" {
        return None;
    }

    let preferred = preferred_tool_names_with_availability(hint, availability);
    if preferred.is_empty() {
        return None;
    }

    let formatted_tools = preferred
        .iter()
        .map(|tool| format!("`{}`", tool))
        .collect::<Vec<_>>();

    Some(if formatted_tools.len() == 1 {
        format!("Preferred tool: {}.", formatted_tools[0])
    } else {
        format!("Preferred tools: {}.", formatted_tools.join(", "))
    })
}

/// Check if a tool_hint is available (builtin or matches a loaded skill).
pub fn is_hint_available(hint: &str, skills: &[LoadedSkill]) -> bool {
    is_builtin_hint(hint)
        || skills.iter().any(|s| {
            s.name == hint
                || s.name.replace('-', "_") == hint.replace('-', "_")
                || s.tool_definitions
                    .iter()
                    .any(|td| td.function.name == hint.replace('-', "_"))
        })
}

/// Check hint availability against the final registry surface.
pub fn is_hint_available_with_availability(
    hint: &str,
    skills: &[LoadedSkill],
    availability: &ToolAvailabilityView,
) -> bool {
    if let Some(builtin) = find_builtin(hint) {
        return builtin.tools.is_empty()
            || builtin.tools.iter().any(|tool| availability.has_tool(tool));
    }

    availability.has_skill_hint(hint)
        || skills.iter().any(|s| {
            (s.name == hint || s.name.replace('-', "_") == hint.replace('-', "_"))
                && s.tool_definitions
                    .iter()
                    .any(|td| availability.has_tool(&td.function.name))
        })
        || availability.has_tool(&hint.replace('-', "_"))
}

/// Generate the "MATCH tool_hint" execution rule line from the registry.
/// This replaces the previously hardcoded mapping in `build_task_system_prompt`.
pub fn generate_match_rule() -> String {
    let mut parts: Vec<String> = BUILTIN_HINTS
        .iter()
        .filter(|b| !b.tools.is_empty())
        .map(|b| {
            let tools_str = b
                .tools
                .iter()
                .map(|t| format!("`{}`", t))
                .collect::<Vec<_>>()
                .join("/");
            format!("`{}` → {}", b.hint, tools_str)
        })
        .collect();
    parts.push("skill name → call that skill".to_string());
    format!("1. **MATCH tool_hint**: {}.", parts.join("; "))
}

/// Generate the execution rule line from the final availability view.
pub fn generate_match_rule_with_availability(availability: &ToolAvailabilityView) -> String {
    let mut parts: Vec<String> = BUILTIN_HINTS
        .iter()
        .filter_map(|builtin| {
            let tools = available_builtin_tools(builtin.hint, availability);
            if tools.is_empty() {
                return None;
            }
            let tools_str = tools
                .iter()
                .map(|tool| format!("`{}`", tool))
                .collect::<Vec<_>>()
                .join("/");
            Some(format!("`{}` → {}", builtin.hint, tools_str))
        })
        .collect();

    if availability.has_any_skills() {
        parts.push("skill name → call that skill".to_string());
    }

    format!("1. **MATCH tool_hint**: {}.", parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::ExtensionRegistry;

    #[test]
    fn preferred_tools_for_known_hints() {
        let tools = preferred_tool_names("file_read");
        assert!(tools.contains(&"read_file".to_string()));
        assert!(tools.contains(&"file_exists".to_string()));
    }

    #[test]
    fn preferred_tools_for_skill_hint() {
        let tools = preferred_tool_names("weather");
        assert_eq!(tools, vec!["weather".to_string()]);
    }

    #[test]
    fn preferred_tools_for_empty_hint() {
        assert!(preferred_tool_names("").is_empty());
    }

    #[test]
    fn analysis_hint_returns_no_tools() {
        assert!(preferred_tool_names("analysis").is_empty());
    }

    #[test]
    fn guidance_for_known_hint() {
        assert!(hint_guidance("file_write")
            .unwrap()
            .contains("write_output"));
    }

    #[test]
    fn guidance_for_unknown_hint_is_none() {
        assert!(hint_guidance("weather").is_none());
    }

    #[test]
    fn generate_match_rule_contains_all_builtin_hints() {
        let rule = generate_match_rule();
        assert!(rule.contains("`file_read`"));
        assert!(rule.contains("`read_file`"));
        assert!(rule.contains("`preview`"));
        assert!(rule.contains("`preview_server`"));
        assert!(rule.contains("skill name → call that skill"));
    }

    #[test]
    fn builtin_hint_availability() {
        assert!(is_hint_available("file_read", &[]));
        assert!(is_hint_available("analysis", &[]));
        assert!(!is_hint_available("weather", &[]));
    }

    #[test]
    fn availability_view_filters_mutating_builtin_hints() {
        let registry = ExtensionRegistry::read_only(true, false, &[]);
        let view = registry.availability();

        assert_eq!(
            preferred_tool_names_with_availability("file_read", view),
            vec!["file_exists".to_string(), "read_file".to_string()]
        );
        assert!(preferred_tool_names_with_availability("file_write", view).is_empty());
        assert!(is_hint_available_with_availability(
            "memory_search",
            &[],
            view
        ));
        assert!(!is_hint_available_with_availability(
            "memory_write",
            &[],
            view
        ));

        let rule = generate_match_rule_with_availability(view);
        assert!(rule.contains("`file_read`"));
        assert!(!rule.contains("`file_write`"));
        assert!(!rule.contains("`run_command`"));
    }

    /// Exhaustive equivalence test: every hint that existed in the old hardcoded
    /// `TaskPlanner` match arms must produce identical results here.
    #[test]
    fn exhaustive_equivalence_with_old_task_planner() {
        fn sorted(mut v: Vec<String>) -> Vec<String> {
            v.sort();
            v.dedup();
            v
        }

        // ── preferred_tool_names (old match arms) ──
        assert_eq!(preferred_tool_names("analysis"), Vec::<String>::new());
        assert_eq!(preferred_tool_names("chat_history"), vec!["chat_history"]);
        assert_eq!(preferred_tool_names("memory_write"), vec!["memory_write"]);
        assert_eq!(
            sorted(preferred_tool_names("memory_search")),
            sorted(vec!["memory_search".into(), "memory_list".into()])
        );
        assert_eq!(
            sorted(preferred_tool_names("file_list")),
            sorted(vec!["list_directory".into(), "file_exists".into()])
        );
        assert_eq!(
            sorted(preferred_tool_names("file_read")),
            sorted(vec!["read_file".into(), "file_exists".into()])
        );
        assert_eq!(
            sorted(preferred_tool_names("file_write")),
            sorted(vec!["write_output".into(), "write_file".into()])
        );
        assert_eq!(
            sorted(preferred_tool_names("file_edit")),
            sorted(vec![
                "read_file".into(),
                "file_exists".into(),
                "search_replace".into(),
                "preview_edit".into(),
                "write_file".into()
            ])
        );
        assert_eq!(preferred_tool_names("preview"), vec!["preview_server"]);
        assert_eq!(preferred_tool_names("command"), vec!["run_command"]);
        assert_eq!(
            sorted(preferred_tool_names("file_operation")),
            sorted(vec![
                "read_file".into(),
                "list_directory".into(),
                "file_exists".into(),
                "write_output".into(),
                "write_file".into(),
                "search_replace".into(),
                "preview_edit".into(),
                "preview_server".into(),
                "run_command".into(),
            ])
        );
        // Unknown skill hint → normalized name
        assert_eq!(
            preferred_tool_names("my-custom-skill"),
            vec!["my_custom_skill"]
        );
        assert_eq!(preferred_tool_names(""), Vec::<String>::new());

        // ── hint_guidance (old match arms) ──
        assert_eq!(
            hint_guidance("file_list").unwrap(),
            "Preferred tools: `list_directory` (and `file_exists` if needed)."
        );
        assert_eq!(
            hint_guidance("file_read").unwrap(),
            "Preferred tools: `read_file` (and `file_exists` if needed)."
        );
        assert!(hint_guidance("file_write")
            .unwrap()
            .starts_with("Preferred tools: `write_output`"));
        assert!(hint_guidance("file_edit")
            .unwrap()
            .contains("search_replace"));
        assert_eq!(
            hint_guidance("preview").unwrap(),
            "Preferred tool: `preview_server`."
        );
        assert_eq!(
            hint_guidance("command").unwrap(),
            "Preferred tool: `run_command`."
        );
        assert_eq!(
            hint_guidance("chat_history").unwrap(),
            "Preferred tool: `chat_history`."
        );
        assert_eq!(
            hint_guidance("memory_write").unwrap(),
            "Preferred tool: `memory_write`."
        );
        assert!(hint_guidance("memory_search")
            .unwrap()
            .contains("memory_search"));
        assert!(hint_guidance("file_operation").unwrap().contains("Legacy"));
        assert!(hint_guidance("analysis").is_none());
        assert!(hint_guidance("unknown_skill").is_none());

        // ── builtin hint names list (old BUILTIN_HINTS const) ──
        let names = builtin_hint_names();
        for expected in &[
            "file_operation",
            "file_list",
            "file_read",
            "file_write",
            "file_edit",
            "preview",
            "command",
            "chat_history",
            "memory_write",
            "memory_search",
            "analysis",
        ] {
            assert!(
                names.contains(expected),
                "missing builtin hint: {}",
                expected
            );
        }
        assert_eq!(names.len(), 11, "should have exactly 11 builtin hints");
    }
}
