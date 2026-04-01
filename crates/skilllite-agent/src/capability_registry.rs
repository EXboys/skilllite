//! Capability registry: explicit map of what the agent can do now.

use std::collections::{BTreeMap, BTreeSet};

use super::skills::LoadedSkill;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityLevel {
    Exploratory,
    Working,
    Strong,
}

impl CapabilityLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Exploratory => "exploratory",
            Self::Working => "working",
            Self::Strong => "strong",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityEntry {
    pub skill_name: String,
    pub domains: Vec<String>,
    pub level: CapabilityLevel,
    pub maturity_score: u8,
    pub callable_tool_count: usize,
    pub reference_only: bool,
    pub known_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityRegistry {
    pub entries: Vec<CapabilityEntry>,
    pub domain_coverage: BTreeMap<String, usize>,
    pub average_maturity: u8,
}

impl CapabilityRegistry {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn covered_domains(&self) -> BTreeSet<String> {
        self.domain_coverage
            .iter()
            .filter_map(|(domain, count)| {
                if *count > 0 {
                    Some(domain.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn to_planning_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let total = self.entries.len();
        let domains: Vec<String> = self.domain_coverage.keys().cloned().collect();
        let mut lines = vec![
            "## Capability Registry (current ability map)".to_string(),
            format!(
                "- **Summary**: {} skill capabilities, {} covered domains, average maturity {}",
                total,
                domains.len(),
                self.average_maturity
            ),
            format!("- **Covered domains**: {}", domains.join(", ")),
            "- **Top capability entries**:".to_string(),
        ];
        for entry in self.entries.iter().take(8) {
            lines.push(format!(
                "  - {} | level={} | maturity={} | tools={} | domains={}",
                entry.skill_name,
                entry.level.as_str(),
                entry.maturity_score,
                entry.callable_tool_count,
                entry.domains.join("/")
            ));
        }
        lines.push(String::new());
        lines.push(
            "Use this registry as capability ground truth. Do not plan tasks that require missing domains without fallback strategy."
                .to_string(),
        );
        lines.join("\n")
    }
}

pub(crate) fn infer_capability_domains(text: &str) -> Vec<String> {
    let s = text.to_lowercase();
    let mut domains = BTreeSet::new();
    let rules: [(&str, &[&str]); 10] = [
        (
            "planning",
            &["plan", "规划", "replan", "task list", "里程碑"],
        ),
        ("analysis", &["analy", "评估", "分析", "reason", "diagnos"]),
        (
            "coding",
            &["code", "refactor", "实现", "修复", "rust", "python"],
        ),
        (
            "testing",
            &["test", "验证", "回归", "clippy", "fmt", "pytest", "cargo"],
        ),
        (
            "filesystem",
            &[
                "file",
                "目录",
                "read_file",
                "write_file",
                "patch",
                "search_replace",
            ],
        ),
        (
            "execution",
            &["run", "command", "shell", "execute", "运行", "终端"],
        ),
        (
            "search",
            &["search", "grep", "rg", "find", "scan", "检索", "搜索"],
        ),
        ("docs", &["doc", "readme", "文档", "说明", "markdown"]),
        (
            "security",
            &["security", "audit", "风险", "sandbox", "权限"],
        ),
        ("memory", &["memory", "记忆", "history", "context"]),
    ];

    for (domain, keys) in rules {
        if keys.iter().any(|k| s.contains(k)) {
            domains.insert(domain.to_string());
        }
    }
    if domains.is_empty() {
        domains.insert("general".to_string());
    }
    domains.into_iter().collect()
}

fn infer_maturity(skill: &LoadedSkill, reference_only: bool) -> (u8, CapabilityLevel) {
    let mut score: u8 = 20;
    let tools_bonus = (skill.tool_definitions.len() as u8)
        .saturating_mul(15)
        .min(40);
    score = score.saturating_add(tools_bonus);
    if !skill.metadata.entry_point.is_empty() {
        score = score.saturating_add(20);
    }
    if skill.metadata.is_bash_tool_skill() {
        score = score.saturating_add(10);
    }
    if skill.multi_script_entries.len() > 1 {
        score = score.saturating_add(10);
    }
    if reference_only {
        score = score.saturating_sub(15);
    }
    let level = if score < 35 {
        CapabilityLevel::Exploratory
    } else if score < 65 {
        CapabilityLevel::Working
    } else {
        CapabilityLevel::Strong
    };
    (score.min(95), level)
}

pub fn build_capability_registry(skills: &[&LoadedSkill]) -> CapabilityRegistry {
    let mut entries = Vec::new();
    let mut coverage: BTreeMap<String, usize> = BTreeMap::new();
    let mut maturity_sum: u64 = 0;

    for skill in skills {
        let reference_only = skill.tool_definitions.is_empty();
        let mut source = format!(
            "{} {} {}",
            skill.name,
            skill.metadata.description.as_deref().unwrap_or(""),
            skill
                .tool_definitions
                .iter()
                .map(|d| d.function.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        for cap in &skill.metadata.capabilities {
            source.push(' ');
            source.push_str(cap);
        }
        let domains = infer_capability_domains(&source);
        let (maturity_score, level) = infer_maturity(skill, reference_only);
        maturity_sum += maturity_score as u64;

        let mut limitations = Vec::new();
        if reference_only {
            limitations.push("reference-only (no callable tool definition)".to_string());
        }
        if skill.metadata.entry_point.is_empty() && !skill.metadata.is_bash_tool_skill() {
            limitations.push("no explicit entry point".to_string());
        }

        for d in &domains {
            let entry = coverage.entry(d.clone()).or_insert(0);
            if !reference_only {
                *entry += 1;
            }
        }

        entries.push(CapabilityEntry {
            skill_name: skill.name.clone(),
            domains,
            level,
            maturity_score,
            callable_tool_count: skill.tool_definitions.len(),
            reference_only,
            known_limitations: limitations,
        });
    }

    let average_maturity = if entries.is_empty() {
        0
    } else {
        (maturity_sum / entries.len() as u64) as u8
    };

    CapabilityRegistry {
        entries,
        domain_coverage: coverage,
        average_maturity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FunctionDef, ToolDefinition};
    use skilllite_core::skill::metadata::{NetworkPolicy, SkillMetadata};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fake_skill(name: &str, desc: &str, tool_count: usize) -> LoadedSkill {
        let metadata = SkillMetadata {
            name: name.to_string(),
            entry_point: if tool_count > 0 {
                "scripts/main.py".to_string()
            } else {
                String::new()
            },
            language: Some("python".to_string()),
            description: Some(desc.to_string()),
            version: None,
            compatibility: None,
            network: NetworkPolicy::default(),
            resolved_packages: None,
            allowed_tools: None,
            requires_elevated_permissions: false,
            capabilities: Vec::new(),
        };
        let tools = (0..tool_count)
            .map(|i| ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: format!("{}_{}", name, i),
                    description: "test".to_string(),
                    parameters: serde_json::json!({"type":"object"}),
                },
            })
            .collect();
        LoadedSkill {
            name: name.to_string(),
            skill_dir: PathBuf::from("."),
            metadata,
            tool_definitions: tools,
            multi_script_entries: HashMap::new(),
        }
    }

    #[test]
    fn build_registry_infers_domains_and_maturity() {
        let s1 = fake_skill("code_helper", "Refactor Rust code and add tests", 2);
        let s2 = fake_skill("docs_helper", "Update README docs", 1);
        let registry = build_capability_registry(&[&s1, &s2]);
        assert!(!registry.is_empty());
        assert!(registry.domain_coverage.contains_key("coding"));
        assert!(registry.domain_coverage.contains_key("testing"));
        assert!(registry.domain_coverage.contains_key("docs"));
        assert!(registry.average_maturity > 0);
    }

    #[test]
    fn reference_only_skill_has_limitation() {
        let s1 = fake_skill("reference_skill", "analysis only docs", 0);
        let registry = build_capability_registry(&[&s1]);
        let first = registry.entries.first().expect("entry exists");
        assert!(first.reference_only);
        assert!(first
            .known_limitations
            .iter()
            .any(|s| s.contains("reference-only")));
    }
}
