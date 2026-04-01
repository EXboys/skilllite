//! Goal contract extraction: normalize user goal into executable fields.
//!
//! Fields:
//! - goal
//! - acceptance
//! - constraints
//! - deadline
//! - risk_level

use regex::Regex;

/// Structured risk level for planning safety and prioritization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub(crate) fn from_text(input: &str) -> Option<Self> {
        let norm = input.trim().to_lowercase();
        if norm.is_empty() {
            return None;
        }
        if norm.contains("critical") || norm.contains("极高") || norm.contains("严重") {
            return Some(Self::Critical);
        }
        if norm.contains("high") || norm.contains("高") {
            return Some(Self::High);
        }
        if norm.contains("medium") || norm.contains("中") {
            return Some(Self::Medium);
        }
        if norm.contains("low") || norm.contains("低") {
            return Some(Self::Low);
        }
        None
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Executable goal contract extracted from user input.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoalContract {
    pub goal: Option<String>,
    pub acceptance: Option<String>,
    pub constraints: Option<String>,
    pub deadline: Option<String>,
    pub risk_level: Option<RiskLevel>,
}

impl GoalContract {
    pub fn is_empty(&self) -> bool {
        self.goal.is_none()
            && self.acceptance.is_none()
            && self.constraints.is_none()
            && self.deadline.is_none()
            && self.risk_level.is_none()
    }

    pub fn to_planning_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut lines = vec!["## Goal Contract (executable)".to_string()];
        if let Some(ref goal) = self.goal {
            lines.push(format!("- **Goal**: {}", goal.trim()));
        }
        if let Some(ref acceptance) = self.acceptance {
            lines.push(format!("- **Acceptance**: {}", acceptance.trim()));
        }
        if let Some(ref constraints) = self.constraints {
            lines.push(format!("- **Constraints**: {}", constraints.trim()));
        }
        if let Some(ref deadline) = self.deadline {
            lines.push(format!("- **Deadline**: {}", deadline.trim()));
        }
        if let Some(ref risk_level) = self.risk_level {
            lines.push(format!("- **Risk level**: {}", risk_level.as_str()));
        }
        lines.push(String::new());
        lines.push(
            "Use this contract to keep planning concrete, verifiable, and risk-aware.".to_string(),
        );
        lines.join("\n")
    }
}

fn extract_first_match(input: &str, patterns: &[&str]) -> Option<String> {
    for pat in patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(input) {
                if let Some(m) = cap.get(1) {
                    let s = m.as_str().trim();
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    None
}

fn fallback_goal(input: &str) -> Option<String> {
    let first_line = input
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    if first_line.is_empty() {
        return None;
    }
    let truncated: String = first_line.chars().take(240).collect();
    Some(truncated)
}

/// Extract goal contract fields from raw user goal text.
pub fn extract_goal_contract(input: &str) -> GoalContract {
    let input = input.trim();
    if input.is_empty() {
        return GoalContract::default();
    }

    let goal = extract_first_match(
        input,
        &[
            r"(?i)目标[：:]\s*([^\n]+)",
            r"(?i)goal[：:]\s*([^\n]+)",
            r"(?i)objective[：:]\s*([^\n]+)",
        ],
    )
    .or_else(|| fallback_goal(input));

    let acceptance = extract_first_match(
        input,
        &[
            r"(?i)验收标准[：:]\s*([^\n]+)",
            r"(?i)验收[：:]\s*([^\n]+)",
            r"(?i)完成标准[：:]\s*([^\n]+)",
            r"(?i)acceptance(?:\s+criteria)?[：:]\s*([^\n]+)",
        ],
    );

    let constraints = extract_first_match(
        input,
        &[
            r"(?i)约束[：:]\s*([^\n]+)",
            r"(?i)限制[：:]\s*([^\n]+)",
            r"(?i)constraints?[：:]\s*([^\n]+)",
        ],
    );

    let deadline = extract_first_match(
        input,
        &[
            r"(?i)截止时间[：:]\s*([^\n]+)",
            r"(?i)截止日期[：:]\s*([^\n]+)",
            r"(?i)deadline[：:]\s*([^\n]+)",
            r"(?i)due\s+date[：:]\s*([^\n]+)",
        ],
    );

    let risk_level = extract_first_match(
        input,
        &[
            r"(?i)风险级别[：:]\s*([^\n]+)",
            r"(?i)风险等级[：:]\s*([^\n]+)",
            r"(?i)risk\s+level[：:]\s*([^\n]+)",
        ],
    )
    .and_then(|s| RiskLevel::from_text(&s));

    GoalContract {
        goal,
        acceptance,
        constraints,
        deadline,
        risk_level,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_goal_contract_cn_fields() {
        let input = "目标：完成 agent 规划重构\n验收：单测通过并输出任务计划\n约束：不新增依赖\n截止时间：2026-04-10\n风险级别：高";
        let c = extract_goal_contract(input);

        assert_eq!(c.goal.as_deref(), Some("完成 agent 规划重构"));
        assert!(c
            .acceptance
            .as_deref()
            .is_some_and(|v| v.contains("单测通过")));
        assert_eq!(c.constraints.as_deref(), Some("不新增依赖"));
        assert_eq!(c.deadline.as_deref(), Some("2026-04-10"));
        assert_eq!(c.risk_level, Some(RiskLevel::High));
    }

    #[test]
    fn extract_goal_contract_empty_input() {
        let c = extract_goal_contract("");
        assert!(c.is_empty());
        assert_eq!(c.to_planning_block(), "");
    }

    #[test]
    fn extract_goal_contract_fallback_goal_and_risk_mapping() {
        let c = extract_goal_contract("请把 README 和 docs 同步。risk level: critical");
        assert!(c.goal.as_deref().is_some_and(|v| v.contains("README")));
        assert_eq!(c.risk_level, Some(RiskLevel::Critical));
    }

    #[test]
    fn to_planning_block_contains_core_fields() {
        let c = GoalContract {
            goal: Some("ship feature".to_string()),
            acceptance: Some("tests pass".to_string()),
            constraints: Some("no new deps".to_string()),
            deadline: Some("Friday".to_string()),
            risk_level: Some(RiskLevel::Medium),
        };
        let block = c.to_planning_block();
        assert!(block.contains("Goal Contract"));
        assert!(block.contains("Acceptance"));
        assert!(block.contains("Risk level"));
    }
}
