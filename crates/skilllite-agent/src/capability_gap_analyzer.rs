//! Capability gap analyzer: compare required domains vs current registry.

use std::collections::BTreeSet;

use super::capability_registry::{infer_capability_domains, CapabilityRegistry};
use super::goal_contract::GoalContract;

#[derive(Debug, Clone, Default)]
pub struct CapabilityGapReport {
    pub required_domains: Vec<String>,
    pub covered_domains: Vec<String>,
    pub missing_domains: Vec<String>,
    pub coverage_ratio: f32,
}

impl CapabilityGapReport {
    pub fn is_empty(&self) -> bool {
        self.required_domains.is_empty()
    }

    pub fn gap_ratio(&self) -> f32 {
        1.0 - self.coverage_ratio
    }

    pub fn to_planning_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let severity = if self.gap_ratio() >= 0.6 {
            "high"
        } else if self.gap_ratio() >= 0.3 {
            "medium"
        } else {
            "low"
        };
        [
            "## Capability Gap Analysis (goal requirements vs current capabilities)".to_string(),
            format!("- **Required domains**: {}", self.required_domains.join(", ")),
            format!("- **Covered domains**: {}", self.covered_domains.join(", ")),
            format!("- **Missing domains**: {}", self.missing_domains.join(", ")),
            format!("- **Coverage ratio**: {:.0}%", self.coverage_ratio * 100.0),
            format!("- **Gap severity**: {}", severity),
            String::new(),
            "Plan with currently covered domains first, and explicitly define fallback paths for missing domains.".to_string(),
        ]
        .join("\n")
    }
}

pub fn analyze_capability_gaps(
    user_message: &str,
    goal_contract: Option<&GoalContract>,
    registry: &CapabilityRegistry,
) -> CapabilityGapReport {
    let mut requirement_text = user_message.to_string();
    if let Some(contract) = goal_contract {
        if let Some(ref goal) = contract.goal {
            requirement_text.push(' ');
            requirement_text.push_str(goal);
        }
        if let Some(ref acceptance) = contract.acceptance {
            requirement_text.push(' ');
            requirement_text.push_str(acceptance);
        }
        if let Some(ref constraints) = contract.constraints {
            requirement_text.push(' ');
            requirement_text.push_str(constraints);
        }
    }

    let required = infer_capability_domains(&requirement_text);
    let required_set: BTreeSet<String> = required.into_iter().collect();
    if required_set.is_empty() {
        return CapabilityGapReport::default();
    }

    let covered_set = registry.covered_domains();
    let covered: Vec<String> = required_set
        .iter()
        .filter_map(|d| {
            if covered_set.contains(d) {
                Some(d.clone())
            } else {
                None
            }
        })
        .collect();
    let missing: Vec<String> = required_set
        .iter()
        .filter_map(|d| {
            if !covered_set.contains(d) {
                Some(d.clone())
            } else {
                None
            }
        })
        .collect();
    let coverage_ratio = if required_set.is_empty() {
        1.0
    } else {
        covered.len() as f32 / required_set.len() as f32
    };
    CapabilityGapReport {
        required_domains: required_set.into_iter().collect(),
        covered_domains: covered,
        missing_domains: missing,
        coverage_ratio,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_registry::build_capability_registry;
    use crate::goal_contract::GoalContract;
    use crate::skills::LoadedSkill;
    use crate::types::{FunctionDef, ToolDefinition};
    use skilllite_core::skill::metadata::{NetworkPolicy, SkillMetadata};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn fake_skill(name: &str, desc: &str) -> LoadedSkill {
        let metadata = SkillMetadata {
            name: name.to_string(),
            entry_point: "scripts/main.py".to_string(),
            language: Some("python".to_string()),
            description: Some(desc.to_string()),
            version: None,
            compatibility: None,
            network: NetworkPolicy::default(),
            resolved_packages: None,
            allowed_tools: None,
            requires_elevated_permissions: false,
            capabilities: Vec::new(),
            openclaw_installs: None,
        };
        LoadedSkill {
            name: name.to_string(),
            skill_dir: PathBuf::from("."),
            metadata,
            tool_definitions: vec![ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDef {
                    name: name.to_string(),
                    description: "test".to_string(),
                    parameters: serde_json::json!({"type":"object"}),
                },
            }],
            multi_script_entries: HashMap::new(),
        }
    }

    #[test]
    fn report_identifies_missing_domains() {
        let skill = fake_skill("doc_tool", "Update docs and markdown files");
        let reg = build_capability_registry(&[&skill]);
        let report = analyze_capability_gaps("Refactor Rust code and run tests", None, &reg);
        assert!(report.required_domains.contains(&"coding".to_string()));
        assert!(report.missing_domains.contains(&"coding".to_string()));
        assert!(report.coverage_ratio < 1.0);
    }

    #[test]
    fn report_uses_goal_contract_context() {
        let skill = fake_skill("test_tool", "Run cargo test and validate regressions");
        let reg = build_capability_registry(&[&skill]);
        let contract = GoalContract {
            acceptance: Some("All tests pass".to_string()),
            ..Default::default()
        };
        let report = analyze_capability_gaps("Improve stability", Some(&contract), &reg);
        assert!(report.required_domains.contains(&"testing".to_string()));
        assert!(report.covered_domains.contains(&"testing".to_string()));
    }
}
