//! Clarification sub-module: reusable clarification-request pattern.
//!
//! Encapsulates the repeated pattern of requesting clarification from the user
//! when the agent hits a stopping condition (max iterations, consecutive
//! failures, tool call limits, etc.).

use super::super::types::*;

/// Maximum number of clarification round-trips before the agent stops unconditionally.
pub(super) const MAX_CLARIFICATIONS: usize = 3;

/// What the caller should do after a clarification attempt.
pub(super) enum ClarifyAction {
    /// User chose to continue; any hint was already pushed to `messages`.
    Continue,
    /// Clarification was declined or the limit was reached.
    Declined,
}

/// Attempt a clarification request with the user.
///
/// If `clarification_count < MAX_CLARIFICATIONS` and the user responds with
/// `Continue`, increments `clarification_count`, pushes the hint (if any) to
/// `messages`, and returns `ClarifyAction::Continue`.
///
/// Otherwise returns `ClarifyAction::Declined`.
pub(super) fn try_clarify(
    reason: &str,
    message: &str,
    suggestions: &[&str],
    clarification_count: &mut usize,
    event_sink: &mut dyn EventSink,
    messages: &mut Vec<ChatMessage>,
) -> ClarifyAction {
    if *clarification_count >= MAX_CLARIFICATIONS {
        return ClarifyAction::Declined;
    }
    let req = ClarificationRequest {
        reason: reason.into(),
        message: message.into(),
        suggestions: suggestions.iter().map(|s| s.to_string()).collect(),
    };
    match event_sink.on_clarification_request(&req) {
        ClarificationResponse::Continue(hint) => {
            *clarification_count += 1;
            if let Some(h) = hint {
                messages.push(ChatMessage::user(&h));
            }
            ClarifyAction::Continue
        }
        ClarificationResponse::Stop => ClarifyAction::Declined,
    }
}
