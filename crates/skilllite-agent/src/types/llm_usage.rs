//! Aggregated LLM token usage from API-reported `usage` fields.

use serde::{Deserialize, Serialize};

/// Single completion usage snapshot (OpenAI-compatible field names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmUsageReport {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl LlmUsageReport {
    pub fn from_counts(prompt_tokens: u64, completion_tokens: u64, total_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

/// Cumulative usage across many LLM calls in one agent run (or session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LlmUsageTotals {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    /// Responses that included a `usage` object.
    pub responses_with_usage: u32,
    /// Completed HTTP responses with no usage (common for some streaming gateways).
    pub responses_without_usage: u32,
}

impl LlmUsageTotals {
    /// Add one API-reported usage, or count a missing report.
    pub fn record(&mut self, usage: Option<LlmUsageReport>) {
        match usage {
            Some(u) => {
                self.prompt_tokens = self.prompt_tokens.saturating_add(u.prompt_tokens);
                self.completion_tokens = self.completion_tokens.saturating_add(u.completion_tokens);
                self.total_tokens = self.total_tokens.saturating_add(u.total_tokens);
                self.responses_with_usage = self.responses_with_usage.saturating_add(1);
            }
            None => {
                self.responses_without_usage = self.responses_without_usage.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_accumulates_and_counts_missing() {
        let mut t = LlmUsageTotals::default();
        t.record(Some(LlmUsageReport::from_counts(10, 5, 15)));
        t.record(None);
        assert_eq!(t.prompt_tokens, 10);
        assert_eq!(t.completion_tokens, 5);
        assert_eq!(t.total_tokens, 15);
        assert_eq!(t.responses_with_usage, 1);
        assert_eq!(t.responses_without_usage, 1);
    }
}
