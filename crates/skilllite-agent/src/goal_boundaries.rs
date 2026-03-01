//! Goal boundary extraction (A5): scope, exclusions, completion conditions.
//!
//! Hybrid approach:
//! - **Regex first**: Fast, high accuracy when user uses explicit markers
//!   (完成条件：、范围：、排除：、scope：、exclude：)
//! - **LLM fallback**: When regex returns empty, set `SKILLLITE_GOAL_LLM_EXTRACT=1`
//!   to extract from natural language (one extra LLM call at planning time).

use regex::Regex;

/// Extracted boundaries from a goal string.
#[derive(Debug, Clone, Default)]
pub struct GoalBoundaries {
    /// Scope: what is in scope for this goal.
    pub scope: Option<String>,
    /// Exclusions: what to avoid or exclude.
    pub exclusions: Option<String>,
    /// Completion conditions: when the task is considered "done".
    pub completion_conditions: Option<String>,
}

impl GoalBoundaries {
    /// Check if any boundary was extracted.
    pub fn is_empty(&self) -> bool {
        self.scope.is_none() && self.exclusions.is_none() && self.completion_conditions.is_none()
    }

    /// Format as a block for injection into planning prompt.
    pub fn to_planning_block(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut lines = vec!["## Goal Boundaries (extracted from user goal)".to_string()];
        if let Some(ref s) = self.scope {
            lines.push(format!("- **Scope**: {}", s.trim()));
        }
        if let Some(ref e) = self.exclusions {
            lines.push(format!("- **Exclusions**: {}", e.trim()));
        }
        if let Some(ref c) = self.completion_conditions {
            lines.push(format!("- **Completion conditions**: {}", c.trim()));
        }
        lines.push(String::new());
        lines.push("Plan and execute within these boundaries. Consider the task DONE only when completion conditions are met.".to_string());
        lines.join("\n")
    }
}

/// Extract boundaries from goal text using heuristic patterns.
///
/// Supports both Chinese and English markers:
/// - Scope: 范围、scope、in scope
/// - Exclusions: 排除、exclude、不要、don't
/// - Completion: 完成条件、完成标准、when done、done when
pub fn extract_goal_boundaries(goal: &str) -> GoalBoundaries {
    let goal = goal.trim();
    if goal.is_empty() {
        return GoalBoundaries::default();
    }

    let mut boundaries = GoalBoundaries::default();

    // Completion conditions (highest priority for long-running)
    let completion_patterns = [
        r"(?i)完成条件[：:]\s*([^\n]+)",
        r"(?i)完成标准[：:]\s*([^\n]+)",
        r"(?i)done\s+when[：:]\s*([^\n]+)",
        r"(?i)when\s+done[：:]\s*([^\n]+)",
        r"(?i)完成即[：:]\s*([^\n]+)",
    ];
    for pat in &completion_patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(goal) {
                if let Some(m) = cap.get(1) {
                    let s = m.as_str().trim();
                    if !s.is_empty() {
                        boundaries.completion_conditions = Some(s.to_string());
                        break;
                    }
                }
            }
        }
    }

    // Scope
    let scope_patterns = [
        r"(?i)范围[：:]\s*([^\n]+)",
        r"(?i)scope[：:]\s*([^\n]+)",
        r"(?i)in\s+scope[：:]\s*([^\n]+)",
    ];
    for pat in &scope_patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(goal) {
                if let Some(m) = cap.get(1) {
                    let s = m.as_str().trim();
                    if !s.is_empty() {
                        boundaries.scope = Some(s.to_string());
                        break;
                    }
                }
            }
        }
    }

    // Exclusions
    let exclusion_patterns = [
        r"(?i)排除[：:]\s*([^\n]+)",
        r"(?i)exclude[：:]\s*([^\n]+)",
        r"(?i)不要[：:]\s*([^\n]+)",
        r"(?i)don'?t\s*[：:]\s*([^\n]+)",
    ];
    for pat in &exclusion_patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(goal) {
                if let Some(m) = cap.get(1) {
                    let s = m.as_str().trim();
                    if !s.is_empty() {
                        boundaries.exclusions = Some(s.to_string());
                        break;
                    }
                }
            }
        }
    }

    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_completion_conditions() {
        let goal = "整理项目文档。完成条件：所有 README 已更新且通过 review。";
        let b = extract_goal_boundaries(goal);
        assert!(b.completion_conditions.is_some());
        assert!(b.completion_conditions.as_ref().unwrap().contains("README"));
    }

    #[test]
    fn test_extract_scope_and_exclusions() {
        let goal = "重构 src/ 目录。范围：仅 src/。排除：不要动 test/。";
        let b = extract_goal_boundaries(goal);
        assert!(b.scope.as_ref().map_or(false, |s| s.contains("src")));
        assert!(b.exclusions.as_ref().map_or(false, |s| s.contains("test")));
    }

    #[test]
    fn test_empty_goal() {
        let b = extract_goal_boundaries("");
        assert!(b.is_empty());
    }

    #[test]
    fn test_to_planning_block() {
        let mut b = GoalBoundaries::default();
        b.completion_conditions = Some("all tests pass".to_string());
        let block = b.to_planning_block();
        assert!(block.contains("Completion conditions"));
        assert!(block.contains("all tests pass"));
    }
}
