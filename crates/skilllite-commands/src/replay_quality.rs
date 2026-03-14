//! Centralized replay result quality assessment.

use serde::Serialize;

use crate::replay::ReplayCaseResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayFailureKind {
    EmptyOrShortPlan,
    IterationLimit,
    HardRuntimeError,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayQualityAssessment {
    pub quality_ok: bool,
    pub effective_success: bool,
    pub failure_kind: Option<ReplayFailureKind>,
    pub reasons: Vec<String>,
}

/// Hard error patterns that indicate a serious system-level failure.
/// These patterns should only match truly fatal errors that prevent task completion,
/// NOT normal operational feedback like command failures or user confirmations.
const HARD_ERROR_PATTERNS: &[&str] = &[
    // True system-level errors
    "memory_limit", // OOM killer triggered
];

/// Checks if the response contains a hard error.
/// Only matches truly fatal errors, not normal operational feedback.
fn contains_hard_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    HARD_ERROR_PATTERNS.iter().any(|pattern| lower.contains(pattern))
}

pub fn assess_replay_quality(result: &ReplayCaseResult) -> ReplayQualityAssessment {
    let mut reasons = Vec::new();
    let mut failure_kind = None;
    let mut quality_ok = true;

    if let Some(error) = &result.error {
        quality_ok = false;
        reasons.push(format!("runner_error: {}", error));
        failure_kind = Some(ReplayFailureKind::HardRuntimeError);
    }

    if contains_hard_error(&result.response_preview) {
        quality_ok = false;
        reasons.push("response_contains_hard_error".to_string());
        failure_kind.get_or_insert(ReplayFailureKind::HardRuntimeError);
    }

    if result.failed_tools > 0 {
        quality_ok = false;
        reasons.push(format!("failed_tools={}", result.failed_tools));
    }

    if !result.success {
        if result.total_tools == 0 || (result.total_tools <= 3 && result.replans == 0) {
            failure_kind.get_or_insert(ReplayFailureKind::EmptyOrShortPlan);
            reasons.push("too_few_tools_for_failed_run".to_string());
        } else if result.total_tools >= 40 {
            failure_kind.get_or_insert(ReplayFailureKind::IterationLimit);
            reasons.push("tool_calls_hit_long_tail_limit".to_string());
        } else {
            failure_kind.get_or_insert(ReplayFailureKind::Unknown);
        }
    }

    ReplayQualityAssessment {
        quality_ok,
        effective_success: result.success && quality_ok,
        failure_kind,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_result() -> ReplayCaseResult {
        ReplayCaseResult {
            id: "case".to_string(),
            success: true,
            first_success: true,
            replans: 0,
            total_tools: 2,
            failed_tools: 0,
            elapsed_ms: 1000,
            response_preview: "ok".to_string(),
            error: None,
            quality_ok: true,
            effective_success: true,
            failure_kind: None,
            quality_reasons: Vec::new(),
        }
    }

    #[test]
    fn test_assess_replay_quality_flags_failed_tools() {
        let mut result = base_result();
        result.failed_tools = 1;
        let assessment = assess_replay_quality(&result);
        assert!(!assessment.quality_ok);
        assert!(!assessment.effective_success);
    }

    #[test]
    fn test_assess_replay_quality_classifies_short_failed_run() {
        let mut result = base_result();
        result.success = false;
        result.first_success = false;
        result.total_tools = 0;
        let assessment = assess_replay_quality(&result);
        assert_eq!(assessment.failure_kind, Some(ReplayFailureKind::EmptyOrShortPlan));
    }

    #[test]
    fn test_hard_error_not_triggered_by_command_failed() {
        // "command failed" should NOT trigger hard error (normal operational feedback)
        let result = ReplayCaseResult {
            response_preview: "Command failed (exit 1).".to_string(),
            ..base_result()
        };
        let assessment = assess_replay_quality(&result);
        assert!(assessment.quality_ok, "command failed should not be a hard error");
    }

    #[test]
    fn test_hard_error_not_triggered_by_security_scan() {
        // Security scan results should NOT trigger hard error (normal confirmation flow)
        let result = ReplayCaseResult {
            response_preview: "Skill 'foo' security scan results: No issues found. Allow execution?".to_string(),
            ..base_result()
        };
        let assessment = assess_replay_quality(&result);
        assert!(assessment.quality_ok, "security scan should not be a hard error");
    }

    #[test]
    fn test_hard_error_triggered_by_memory_limit() {
        // True OOM error should trigger hard error
        let result = ReplayCaseResult {
            response_preview: "memory_limit exceeded".to_string(),
            ..base_result()
        };
        let assessment = assess_replay_quality(&result);
        assert!(!assessment.quality_ok, "memory_limit should be a hard error");
    }
}
