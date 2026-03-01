//! Seed data management for the self-evolving engine (EVO-2 + EVO-6).

use std::path::{Path, PathBuf};

use skilllite_core::planning::{PlanningRule, SourceRegistry};

const SEED_VERSION: u32 = 1;

const SEED_RULES: &str = include_str!("rules.seed.json");
const SEED_SOURCES: &str = include_str!("sources.seed.json");
const SEED_SYSTEM: &str = include_str!("system.seed.md");
const SEED_PLANNING: &str = include_str!("planning.seed.md");
const SEED_EXECUTION: &str = include_str!("execution.seed.md");
const SEED_EXAMPLES: &str = include_str!("examples.seed.md");

fn prompts_dir(chat_root: &Path) -> PathBuf {
    chat_root.join("prompts")
}

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
        write_seed_file(&dir, "rules.json", SEED_RULES);
        write_seed_file(&dir, "sources.json", SEED_SOURCES);
        write_seed_file(&dir, "system.md", SEED_SYSTEM);
        write_seed_file(&dir, "planning.md", SEED_PLANNING);
        write_seed_file(&dir, "execution.md", SEED_EXECUTION);
        write_seed_file(&dir, "examples.md", SEED_EXAMPLES);
    } else {
        merge_seed_rules(&dir);
        merge_seed_sources(&dir);
        write_if_unchanged(&dir, "system.md", SEED_SYSTEM);
        write_if_unchanged(&dir, "planning.md", SEED_PLANNING);
        write_if_unchanged(&dir, "execution.md", SEED_EXECUTION);
        write_if_unchanged(&dir, "examples.md", SEED_EXAMPLES);
    }

    let _ = std::fs::write(&version_file, SEED_VERSION.to_string());
    tracing::info!("Seed data v{} written to {}", SEED_VERSION, dir.display());
}

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

fn write_if_unchanged(dir: &Path, name: &str, new_content: &str) {
    let path = dir.join(name);
    if !path.exists() {
        write_seed_file(dir, name, new_content);
        return;
    }
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing.trim() == new_content.trim() {
            return;
        }
    }
    write_seed_file(dir, name, new_content);
}

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
        if let Some(existing_rule) = merged.iter_mut().find(|r| r.id == seed_rule.id && !r.mutable) {
            *existing_rule = seed_rule.clone();
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&merged) {
        write_seed_file(dir, "rules.json", &json);
    }
}

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

pub fn load_rules(chat_root: &Path) -> Vec<PlanningRule> {
    let path = prompts_dir(chat_root).join("rules.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(rules) = serde_json::from_str::<Vec<PlanningRule>>(&content) {
                if !rules.is_empty() {
                    return rules;
                }
            }
        }
    }
    serde_json::from_str(SEED_RULES).unwrap_or_default()
}

pub fn load_sources(chat_root: &Path) -> SourceRegistry {
    let path = prompts_dir(chat_root).join("sources.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(registry) = serde_json::from_str::<SourceRegistry>(&content) {
                if !registry.sources.is_empty() {
                    return registry;
                }
            }
        }
    }
    serde_json::from_str(SEED_SOURCES).unwrap_or_else(|_| SourceRegistry {
        version: 1,
        sources: Vec::new(),
    })
}

pub fn load_system_prompt(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "system.md", SEED_SYSTEM)
}

pub fn load_planning_template(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "planning.md", SEED_PLANNING)
}

pub fn load_execution_template(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "execution.md", SEED_EXECUTION)
}

pub fn load_examples(chat_root: &Path) -> String {
    load_prompt_file(chat_root, "examples.md", SEED_EXAMPLES)
}

pub fn required_placeholders(name: &str) -> &'static [&'static str] {
    match name {
        "planning.md" => &["{{TODAY}}", "{{RULES_SECTION}}", "{{EXAMPLES_SECTION}}", "{{OUTPUT_DIR}}"],
        "execution.md" => &["{{TODAY}}", "{{SKILLS_LIST}}", "{{OUTPUT_DIR}}"],
        "system.md" => &[],
        "examples.md" => &[],
        _ => &[],
    }
}

pub fn validate_template(name: &str, content: &str) -> Vec<&'static str> {
    required_placeholders(name)
        .iter()
        .filter(|p| !content.contains(**p))
        .copied()
        .collect()
}

pub fn load_prompt_file_with_project(
    chat_root: &Path,
    workspace: Option<&Path>,
    name: &str,
    fallback: &str,
) -> String {
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
                        "Template {} is missing placeholders {:?}",
                        path.display(),
                        missing
                    );
                }
                return content;
            }
        }
    }
    fallback.to_string()
}
