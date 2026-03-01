//! Seed data management for the self-evolving engine (EVO-2 + EVO-6).
//!
//! Compiled-in seed data → `~/.skilllite/chat/prompts/` on first run.
//! After that, runtime reads from disk (user-editable).
//! Seed version tracking prevents overwriting user edits on upgrade.

use std::path::{Path, PathBuf};

use super::super::types::{PlanningRule, SourceEntry, SourceRegistry};

/// Bump this when seed data changes to trigger re-seeding on upgrade.
const SEED_VERSION: u32 = 1;

// Compiled-in seed data (include_str! embeds file content into the binary).
const SEED_RULES: &str = include_str!("../seed/rules.seed.json");
const SEED_SOURCES: &str = include_str!("../seed/sources.seed.json");
const SEED_SYSTEM: &str = include_str!("../seed/system.seed.md");
const SEED_PLANNING: &str = include_str!("../seed/planning.seed.md");
const SEED_EXECUTION: &str = include_str!("../seed/execution.seed.md");
const SEED_EXAMPLES: &str = include_str!("../seed/examples.seed.md");

fn prompts_dir(chat_root: &Path) -> PathBuf {
    chat_root.join("prompts")
}

/// Ensure seed data is written to disk. Idempotent; skips if version matches.
///
/// Called once at startup from `ChatSession::new` or similar entry point.
pub fn ensure_seed_data(chat_root: &Path) {
    let dir = prompts_dir(chat_root);
    let version_file = dir.join(".seed_version");

    let current_version = std::fs::read_to_string(&version_file)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    if current_version >= SEED_VERSION {
        return;
    }

    if std::fs::create_dir_all(&dir).is_err() {
        tracing::warn!("Failed to create prompts dir: {}", dir.display());
        return;
    }

    let rules_exist = dir.join("rules.json").exists();
    if !rules_exist {
        // First install: write all seed files
        write_seed_file(&dir, "rules.json", SEED_RULES);
        write_seed_file(&dir, "sources.json", SEED_SOURCES);
        write_seed_file(&dir, "system.md", SEED_SYSTEM);
        write_seed_file(&dir, "planning.md", SEED_PLANNING);
        write_seed_file(&dir, "execution.md", SEED_EXECUTION);
        write_seed_file(&dir, "examples.md", SEED_EXAMPLES);
    } else {
        // Upgrade: merge seed rules/sources with existing (preserve user edits).
        merge_seed_rules(&dir);
        merge_seed_sources(&dir);
        // Overwrite templates only if they haven't been customized.
        write_if_unchanged(&dir, "system.md", SEED_SYSTEM);
        write_if_unchanged(&dir, "planning.md", SEED_PLANNING);
        write_if_unchanged(&dir, "execution.md", SEED_EXECUTION);
        write_if_unchanged(&dir, "examples.md", SEED_EXAMPLES);
    }

    let _ = std::fs::write(&version_file, SEED_VERSION.to_string());
    tracing::info!("Seed data v{} written to {}", SEED_VERSION, dir.display());
}

/// Force re-seed: overwrite all prompt files with compiled-in seed data.
/// Used by `skilllite evolution reset` to return to factory state.
pub fn ensure_seed_data_force(chat_root: &Path) {
    let dir = prompts_dir(chat_root);
    if std::fs::create_dir_all(&dir).is_err() {
        tracing::warn!("Failed to create prompts dir: {}", dir.display());
        return;
    }
    write_seed_file(&dir, "rules.json", SEED_RULES);
    write_seed_file(&dir, "sources.json", SEED_SOURCES);
    write_seed_file(&dir, "system.md", SEED_SYSTEM);
    write_seed_file(&dir, "planning.md", SEED_PLANNING);
    write_seed_file(&dir, "execution.md", SEED_EXECUTION);
    write_seed_file(&dir, "examples.md", SEED_EXAMPLES);
    let _ = std::fs::write(dir.join(".seed_version"), SEED_VERSION.to_string());
    tracing::info!("Seed data force-reset to v{}", SEED_VERSION);
}

fn write_seed_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Err(e) = std::fs::write(&path, content) {
        tracing::warn!("Failed to write seed file {}: {}", path.display(), e);
    }
}

/// Write seed file only if the existing file matches the PREVIOUS seed version's content.
/// If user has customized the file, skip.
fn write_if_unchanged(dir: &Path, name: &str, new_content: &str) {
    let path = dir.join(name);
    if !path.exists() {
        write_seed_file(dir, name, new_content);
        return;
    }
    // If file was modified by user (different from any known seed), preserve it.
    // Simple heuristic: if it already matches new content, skip; otherwise write.
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing.trim() == new_content.trim() {
            return;
        }
    }
    // For templates, we overwrite on upgrade. Users can re-customize after.
    write_seed_file(dir, name, new_content);
}

/// Merge seed rules into existing rules.json, preserving user-added and evolved rules.
fn merge_seed_rules(dir: &Path) {
    let rules_path = dir.join("rules.json");
    let existing: Vec<PlanningRule> = if rules_path.exists() {
        std::fs::read_to_string(&rules_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let seed: Vec<PlanningRule> = serde_json::from_str(SEED_RULES).unwrap_or_default();

    let mut merged = existing.clone();
    for seed_rule in &seed {
        let exists = merged.iter().any(|r| r.id == seed_rule.id);
        if !exists {
            merged.push(seed_rule.clone());
        }
        // Existing seed rules (mutable=false) get updated content from new seed
        if let Some(existing_rule) = merged.iter_mut().find(|r| r.id == seed_rule.id && !r.mutable) {
            *existing_rule = seed_rule.clone();
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&merged) {
        write_seed_file(dir, "rules.json", &json);
    }
}

/// Merge seed sources into existing sources.json, preserving evolved/user sources.
fn merge_seed_sources(dir: &Path) {
    let sources_path = dir.join("sources.json");
    let seed_registry: SourceRegistry = match serde_json::from_str(SEED_SOURCES) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse SEED_SOURCES: {}", e);
            return;
        }
    };

    let mut existing_registry: SourceRegistry = if sources_path.exists() {
        std::fs::read_to_string(&sources_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| SourceRegistry { version: 1, sources: Vec::new() })
    } else {
        SourceRegistry { version: 1, sources: Vec::new() }
    };

    for seed_src in &seed_registry.sources {
        let already_exists = existing_registry.sources.iter().any(|s| s.id == seed_src.id);
        if !already_exists {
            existing_registry.sources.push(seed_src.clone());
        }
        // Immutable seed sources get their static fields updated on upgrade,
        // but runtime counters (fetch counts, accessibility, etc.) are preserved.
        if let Some(existing) = existing_registry
            .sources
            .iter_mut()
            .find(|s| s.id == seed_src.id && !s.mutable)
        {
            existing.name = seed_src.name.clone();
            existing.url = seed_src.url.clone();
            existing.source_type = seed_src.source_type.clone();
            existing.parser = seed_src.parser.clone();
            existing.region = seed_src.region.clone();
            existing.language = seed_src.language.clone();
            existing.domains = seed_src.domains.clone();
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&existing_registry) {
        write_seed_file(dir, "sources.json", &json);
    }
}

// ─── Public loaders ─────────────────────────────────────────────────────────

/// Load planning rules from disk, falling back to compiled-in seed.
pub fn load_rules(chat_root: &Path) -> Vec<PlanningRule> {
    let path = prompts_dir(chat_root).join("rules.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(rules) = serde_json::from_str::<Vec<PlanningRule>>(&content) {
                if !rules.is_empty() {
                    tracing::debug!("Loaded {} rules from {}", rules.len(), path.display());
                    return rules;
                }
            }
        }
    }
    // Fallback to compiled-in seed
    serde_json::from_str(SEED_RULES).unwrap_or_default()
}

/// Load source registry from disk, falling back to compiled-in seed.
pub fn load_sources(chat_root: &Path) -> SourceRegistry {
    let path = prompts_dir(chat_root).join("sources.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(registry) = serde_json::from_str::<SourceRegistry>(&content) {
                if !registry.sources.is_empty() {
                    tracing::debug!(
                        "Loaded {} sources from {}",
                        registry.sources.len(),
                        path.display()
                    );
                    return registry;
                }
            }
        }
    }
    // Fallback to compiled-in seed
    serde_json::from_str(SEED_SOURCES).unwrap_or_else(|_| SourceRegistry {
        version: 1,
        sources: Vec::new(),
    })
}

/// Load the system prompt template from disk, falling back to compiled-in seed.
pub fn load_system_prompt(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "system.md", SEED_SYSTEM)
}

/// Load the planning prompt template from disk, falling back to compiled-in seed.
pub fn load_planning_template(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "planning.md", SEED_PLANNING)
}

/// Load the execution prompt template from disk, falling back to compiled-in seed.
pub fn load_execution_template(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "execution.md", SEED_EXECUTION)
}

/// Load the examples text from disk, falling back to compiled-in seed.
pub fn load_examples(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "examples.md", SEED_EXAMPLES)
}

/// Required placeholders per template name. Used for two things:
/// 1. On load: warn (but still use) if user-edited template is missing any.
/// 2. On evolution write: reject the write if the new content is missing any.
pub fn required_placeholders(name: &str) -> &'static [&'static str] {
    match name {
        "planning.md" => &["{{TODAY}}", "{{RULES_SECTION}}", "{{EXAMPLES_SECTION}}", "{{OUTPUT_DIR}}"],
        "execution.md" => &["{{TODAY}}", "{{SKILLS_LIST}}", "{{OUTPUT_DIR}}"],
        "system.md" => &[],
        "examples.md" => &[],
        _ => &[],
    }
}

/// Validate that content contains all required placeholders for a template.
/// Returns the list of missing placeholders (empty = valid).
pub fn validate_template(name: &str, content: &str) -> Vec<&'static str> {
    required_placeholders(name)
        .iter()
        .filter(|p| !content.contains(**p))
        .copied()
        .collect()
}

/// EVO-5: Load a prompt file with project-level override support.
///
/// Resolution order:
/// 1. Project-level: `{workspace}/.skilllite/prompts/{name}` (if exists)
/// 2. Global: `~/.skilllite/chat/prompts/{name}`
/// 3. Compiled-in seed (fallback)
pub fn load_prompt_file_with_project(
    chat_root: &Path,
    workspace: Option<&Path>,
    name: &str,
    fallback: &str,
) -> String {
    // Project-level override takes priority
    if let Some(ws) = workspace {
        let project_path = ws.join(".skilllite").join("prompts").join(name);
        if project_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&project_path) {
                if !content.trim().is_empty() {
                    let missing = validate_template(name, &content);
                    if !missing.is_empty() {
                        tracing::warn!(
                            "Project template {} is missing placeholders {:?}",
                            project_path.display(),
                            missing
                        );
                    }
                    tracing::debug!("Using project-level {}", project_path.display());
                    return content;
                }
            }
        }
    }
    load_prompt_file(chat_root, name, fallback)
}

fn load_prompt_file(chat_root: &Path, name: &str, fallback: &str) -> String {
    let path = prompts_dir(chat_root).join(name);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if !content.trim().is_empty() {
                let missing = validate_template(name, &content);
                if !missing.is_empty() {
                    tracing::warn!(
                        "Template {} is missing placeholders {:?} — those sections won't be injected. \
                         Add them back or delete the file to reset to defaults.",
                        path.display(),
                        missing
                    );
                }
                return content;
            }
        }
    }
    // Only fall back to seed if file doesn't exist or is empty
    fallback.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_ensure_seed_data_creates_files() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);

        let dir = prompts_dir(chat_root);
        assert!(dir.join("rules.json").exists());
        assert!(dir.join("system.md").exists());
        assert!(dir.join("planning.md").exists());
        assert!(dir.join("execution.md").exists());
        assert!(dir.join("examples.md").exists());
        assert!(dir.join(".seed_version").exists());

        let version: u32 = std::fs::read_to_string(dir.join(".seed_version"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(version, SEED_VERSION);
    }

    #[test]
    fn test_ensure_seed_data_idempotent() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);
        let first_rules = std::fs::read_to_string(prompts_dir(chat_root).join("rules.json")).unwrap();

        ensure_seed_data(chat_root);
        let second_rules = std::fs::read_to_string(prompts_dir(chat_root).join("rules.json")).unwrap();

        assert_eq!(first_rules, second_rules);
    }

    #[test]
    fn test_load_rules_from_disk() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);
        let rules = load_rules(chat_root);
        assert!(!rules.is_empty());
        assert!(rules.iter().all(|r| r.origin == "seed"));
        assert!(rules.iter().all(|r| !r.mutable));
    }

    #[test]
    fn test_load_rules_fallback_without_seeding() {
        let tmp = TempDir::new().unwrap();
        let rules = load_rules(tmp.path());
        assert!(!rules.is_empty(), "should fall back to compiled-in seed");
    }

    #[test]
    fn test_load_system_prompt() {
        let tmp = TempDir::new().unwrap();
        ensure_seed_data(tmp.path());
        let prompt = load_system_prompt(tmp.path());
        assert!(prompt.contains("helpful AI assistant"));
    }

    #[test]
    fn test_load_planning_template() {
        let tmp = TempDir::new().unwrap();
        ensure_seed_data(tmp.path());
        let template = load_planning_template(tmp.path());
        assert!(template.contains("{{TODAY}}"));
        assert!(template.contains("{{RULES_SECTION}}"));
    }

    #[test]
    fn test_load_execution_template() {
        let tmp = TempDir::new().unwrap();
        ensure_seed_data(tmp.path());
        let template = load_execution_template(tmp.path());
        assert!(template.contains("{{OUTPUT_DIR}}"));
        assert!(template.contains("{{SKILLS_LIST}}"));
    }

    #[test]
    fn test_seed_preserves_user_rules_on_upgrade() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);

        // Simulate user adding a custom rule
        let dir = prompts_dir(chat_root);
        let mut rules = load_rules(chat_root);
        rules.push(PlanningRule {
            id: "user_custom_rule".into(),
            priority: 80,
            keywords: vec!["custom".into()],
            context_keywords: vec![],
            tool_hint: None,
            instruction: "Custom user rule".into(),
            mutable: true,
            origin: "user".into(),
            reusable: false,
            effectiveness: None,
            trigger_count: None,
        });
        let json = serde_json::to_string_pretty(&rules).unwrap();
        std::fs::write(dir.join("rules.json"), json).unwrap();

        // Simulate upgrade: reset version to trigger re-seed
        std::fs::write(dir.join(".seed_version"), "0").unwrap();
        ensure_seed_data(chat_root);

        let merged = load_rules(chat_root);
        assert!(merged.iter().any(|r| r.id == "user_custom_rule"), "user rule should be preserved");
        assert!(merged.iter().any(|r| r.id.starts_with("seed_")), "seed rules should exist");
    }

    #[test]
    fn test_template_respects_user_edit_even_with_missing_placeholder() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);

        // User edits the template and removes {{RULES_SECTION}} (perhaps by accident)
        let dir = prompts_dir(chat_root);
        let custom_template = "You are MY custom planner.\n\nDo stuff for {{TODAY}}.\nOutput: {{OUTPUT_DIR}}\nExamples: {{EXAMPLES_SECTION}}\n";
        std::fs::write(dir.join("planning.md"), custom_template).unwrap();

        // Load should STILL use the user's file (warn but don't discard)
        let loaded = load_planning_template(chat_root);
        assert!(
            loaded.contains("MY custom planner"),
            "must respect user's file even with missing placeholders"
        );
        assert!(
            !loaded.contains("{{RULES_SECTION}}"),
            "the user removed it, so it should be absent"
        );
    }

    #[test]
    fn test_template_fallback_only_when_empty_or_missing() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        // No seed data written — file doesn't exist
        let loaded = load_planning_template(chat_root);
        assert!(
            loaded.contains("{{RULES_SECTION}}"),
            "should fall back to seed when file doesn't exist"
        );

        // Write an empty file
        let dir = prompts_dir(chat_root);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("planning.md"), "   \n  ").unwrap();
        let loaded = load_planning_template(chat_root);
        assert!(
            loaded.contains("{{RULES_SECTION}}"),
            "should fall back to seed when file is blank"
        );
    }

    #[test]
    fn test_template_accepts_valid_customization() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        ensure_seed_data(chat_root);

        // Add custom content while keeping all placeholders
        let dir = prompts_dir(chat_root);
        let mut template = std::fs::read_to_string(dir.join("planning.md")).unwrap();
        template.push_str("\n\n## Custom user section\nAlways respond in Chinese.\n");
        std::fs::write(dir.join("planning.md"), &template).unwrap();

        let loaded = load_planning_template(chat_root);
        assert!(
            loaded.contains("Custom user section"),
            "should accept valid customization that preserves all placeholders"
        );
    }

    #[test]
    fn test_validate_template() {
        let missing = validate_template("planning.md", "just text, no placeholders");
        assert_eq!(missing.len(), 4, "planning.md has 4 required placeholders");

        let complete = "{{TODAY}} {{RULES_SECTION}} {{EXAMPLES_SECTION}} {{OUTPUT_DIR}}";
        let missing = validate_template("planning.md", complete);
        assert!(missing.is_empty(), "all placeholders present");

        let missing = validate_template("rules.json", "anything");
        assert!(missing.is_empty(), "data files have no required placeholders");
    }

    // ─── EVO-5 tests ────────────────────────────────────────────────────────

    #[test]
    fn test_project_level_override() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();

        ensure_seed_data(global.path());

        // Create project-level system.md override
        let proj_prompts = project.path().join(".skilllite").join("prompts");
        std::fs::create_dir_all(&proj_prompts).unwrap();
        std::fs::write(
            proj_prompts.join("system.md"),
            "You are a PROJECT-SPECIFIC assistant.",
        )
        .unwrap();

        // Without project override → global seed
        let loaded = load_prompt_file_with_project(
            global.path(),
            None,
            "system.md",
            SEED_SYSTEM,
        );
        assert!(
            loaded.contains("helpful AI assistant"),
            "without project, should use global"
        );

        // With project override → project takes priority
        let loaded = load_prompt_file_with_project(
            global.path(),
            Some(project.path()),
            "system.md",
            SEED_SYSTEM,
        );
        assert!(
            loaded.contains("PROJECT-SPECIFIC"),
            "project override should take priority"
        );
    }

    #[test]
    fn test_project_level_fallthrough() {
        let global = TempDir::new().unwrap();
        let project = TempDir::new().unwrap();

        ensure_seed_data(global.path());

        // Project has no prompts/ directory — should fall through to global
        let loaded = load_prompt_file_with_project(
            global.path(),
            Some(project.path()),
            "system.md",
            SEED_SYSTEM,
        );
        assert!(
            loaded.contains("helpful AI assistant"),
            "should fall through to global when project has no override"
        );
    }

    #[test]
    fn test_ensure_seed_data_force() {
        let tmp = TempDir::new().unwrap();
        ensure_seed_data(tmp.path());

        // Modify rules.json
        let rules_path = prompts_dir(tmp.path()).join("rules.json");
        std::fs::write(&rules_path, r#"[{"id":"custom","description":"custom"}]"#).unwrap();

        // Force re-seed should overwrite
        ensure_seed_data_force(tmp.path());
        let content = std::fs::read_to_string(&rules_path).unwrap();
        assert!(
            !content.contains("custom"),
            "force re-seed should overwrite user changes"
        );
    }
}
