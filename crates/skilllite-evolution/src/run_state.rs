//! Global evolution run mutex and result type.

use std::sync::atomic::{AtomicBool, Ordering};

// ─── Concurrency: evolution mutex ────────────────────────────────────────────

static EVOLUTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn try_start_evolution() -> bool {
    EVOLUTION_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn finish_evolution() {
    EVOLUTION_IN_PROGRESS.store(false, Ordering::SeqCst);
}

/// Result of attempting to run evolution. Distinguishes "skipped (busy)" from "no scope" from "ran (with or without changes)".
#[derive(Debug, Clone)]
pub enum EvolutionRunResult {
    /// Another evolution run was already in progress; this invocation did not run.
    SkippedBusy,
    /// No evolution scope (e.g. thresholds not met, or evolution disabled).
    NoScope,
    /// Evolution ran. `Some(txn_id)` if changes were produced, `None` if run completed with no changes.
    Completed(Option<String>),
}

impl EvolutionRunResult {
    /// Returns the txn_id if evolution completed with changes.
    pub fn txn_id(&self) -> Option<&str> {
        match self {
            Self::Completed(Some(id)) => Some(id.as_str()),
            _ => None,
        }
    }
}
