//! Process shutdown hook for evolution metrics.

use std::path::Path;

use crate::feedback;
use crate::run_state::{finish_evolution, try_start_evolution};

// ─── Shutdown hook ────────────────────────────────────────────────────────────

pub fn on_shutdown(chat_root: &Path) {
    if !try_start_evolution() {
        return;
    }
    if let Ok(conn) = feedback::open_evolution_db(chat_root) {
        let _ = feedback::update_daily_metrics(&conn);
        // let _ = feedback::export_decisions_md(&conn, &chat_root.join("DECISIONS.md")); // Removed for refactor
    }
    finish_evolution();
}
