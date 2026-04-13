//! A9 growth scheduling: periodic tick, weighted signal window, sweep, min gap between runs.
//!
//! Shared by desktop Life Pulse and agent-side triggers so behavior stays consistent.

use rusqlite::{params, Connection};
use skilllite_core::config::env_keys::evolution as evo_keys;

use crate::feedback::{self, EVOLUTION_LOG_TYPE_RUN_MATERIAL};
use crate::Result;

/// Parsed A9 trigger configuration (env-driven).
#[derive(Debug, Clone)]
pub struct GrowthScheduleConfig {
    /// Periodic arm: minimum seconds between periodic spawn attempts (Life Pulse / agent tick).
    pub interval_secs: u64,
    /// Weighted sum of recent unprocessed meaningful decisions must reach this (signal arm).
    pub weighted_min: i64,
    /// How many latest unprocessed meaningful decisions participate in the weighted sum.
    pub signal_window: i64,
    /// If no material `evolution_run` for this many seconds and weighted sum ≥ 1, allow a low-priority sweep trigger.
    pub sweep_interval_secs: u64,
    /// Minimum seconds since last material `evolution_log` row (`type = evolution_run`) before another autorun (0 = off).
    pub min_run_gap_secs: u64,
    /// OR arm: raw `evolved = 0` count ≥ this also triggers (includes zero-tool rows).
    pub raw_unprocessed_threshold: i64,
}

fn parse_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

impl GrowthScheduleConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let signal_window = parse_i64(evo_keys::SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW, 10).max(1);
        Self {
            interval_secs: parse_u64(evo_keys::SKILLLITE_EVOLUTION_INTERVAL_SECS, 600),
            weighted_min: parse_i64(evo_keys::SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN, 3).max(1),
            signal_window,
            sweep_interval_secs: parse_u64(evo_keys::SKILLLITE_EVO_SWEEP_INTERVAL_SECS, 86_400),
            min_run_gap_secs: parse_u64(evo_keys::SKILLLITE_EVO_MIN_RUN_GAP_SEC, 0),
            raw_unprocessed_threshold: parse_i64(
                evo_keys::SKILLLITE_EVOLUTION_DECISION_THRESHOLD,
                10,
            )
            .max(1),
        }
    }
}

/// Seconds since the latest material `evolution_run` log row, if any (`evolution_run_noop` ignored).
pub fn seconds_since_last_evolution_run(conn: &Connection) -> Result<Option<i64>> {
    let row = conn.query_row(
        "SELECT CAST((julianday('now') - julianday(MAX(ts))) * 86400 AS INTEGER)
         FROM evolution_log WHERE type = ?1",
        params![EVOLUTION_LOG_TYPE_RUN_MATERIAL],
        |r| r.get::<_, Option<i64>>(0),
    );
    match row {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Sum of weights for the latest `window` unprocessed decisions with `total_tools >= 1`.
/// Weight 2 when `feedback = neg` or `failed_tools > 0`, else 1.
pub fn weighted_unprocessed_signal_sum(conn: &Connection, window: i64) -> Result<i64> {
    let window = window.max(1);
    let sum: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(w), 0) FROM (
                SELECT CASE WHEN feedback = 'neg' OR failed_tools > 0 THEN 2 ELSE 1 END AS w
                FROM decisions
                WHERE evolved = 0 AND total_tools >= 1
                ORDER BY id DESC
                LIMIT {}
            )",
            window
        ),
        [],
        |r| r.get(0),
    )?;
    Ok(sum)
}

fn min_run_gap_satisfied(conn: &Connection, gap_secs: u64) -> Result<bool> {
    if gap_secs == 0 {
        return Ok(true);
    }
    match seconds_since_last_evolution_run(conn)? {
        None => Ok(true),
        Some(s) => Ok(s >= gap_secs.min(i64::MAX as u64) as i64),
    }
}

/// Burst / signal arms only (no periodic clock). Used after each chat turn with tools.
pub fn signal_burst_due(conn: &Connection, cfg: &GrowthScheduleConfig) -> Result<bool> {
    if !min_run_gap_satisfied(conn, cfg.min_run_gap_secs)? {
        return Ok(false);
    }
    let weighted = weighted_unprocessed_signal_sum(conn, cfg.signal_window)?;
    let raw = feedback::count_unprocessed_decisions(conn)?;
    let need_signal = weighted >= cfg.weighted_min || raw >= cfg.raw_unprocessed_threshold;
    let need_sweep = match seconds_since_last_evolution_run(conn)? {
        None => false,
        Some(secs_since) => secs_since >= cfg.sweep_interval_secs as i64 && weighted >= 1,
    };
    Ok(need_signal || need_sweep)
}

/// Result of [`growth_due`]: whether an autorun tick is due and whether only the periodic arm fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GrowthDueOutcome {
    pub due: bool,
    /// `true` when [`Self::due`] and the tick is explained solely by the periodic interval (no signal or sweep arm).
    pub periodic_only: bool,
}

/// Full A9 due check including periodic arm. Updates `last_periodic_spawn_unix` when the periodic arm fires.
pub fn growth_due(
    conn: &Connection,
    now_unix: i64,
    last_periodic_spawn_unix: &mut Option<i64>,
    cfg: &GrowthScheduleConfig,
) -> Result<GrowthDueOutcome> {
    if !min_run_gap_satisfied(conn, cfg.min_run_gap_secs)? {
        return Ok(GrowthDueOutcome::default());
    }

    let weighted = weighted_unprocessed_signal_sum(conn, cfg.signal_window)?;
    let raw = feedback::count_unprocessed_decisions(conn)?;
    let need_signal = weighted >= cfg.weighted_min || raw >= cfg.raw_unprocessed_threshold;
    let need_sweep = match seconds_since_last_evolution_run(conn)? {
        None => false,
        Some(secs_since) => secs_since >= cfg.sweep_interval_secs as i64 && weighted >= 1,
    };

    let last_ts = match *last_periodic_spawn_unix {
        None => {
            *last_periodic_spawn_unix = Some(now_unix);
            now_unix
        }
        Some(t) => t,
    };
    let need_periodic = now_unix.saturating_sub(last_ts) >= cfg.interval_secs as i64;

    if !need_signal && !need_sweep && !need_periodic {
        return Ok(GrowthDueOutcome::default());
    }
    if need_periodic {
        *last_periodic_spawn_unix = Some(now_unix);
    }
    let periodic_only = need_periodic && !need_signal && !need_sweep;
    Ok(GrowthDueOutcome {
        due: true,
        periodic_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feedback::EVOLUTION_LOG_TYPE_RUN_MATERIAL;
    use rusqlite::{params, Connection};

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        feedback::ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn weighted_sum_respects_window_and_weights() {
        let conn = open_mem();
        for _ in 0..3 {
            conn.execute(
                "INSERT INTO decisions (evolved, total_tools, failed_tools, feedback)
                 VALUES (0, 2, 0, 'neutral')",
                [],
            )
            .unwrap();
        }
        assert_eq!(weighted_unprocessed_signal_sum(&conn, 10).unwrap(), 3);
        assert_eq!(weighted_unprocessed_signal_sum(&conn, 2).unwrap(), 2);

        conn.execute(
            "INSERT INTO decisions (evolved, total_tools, failed_tools, feedback)
             VALUES (0, 1, 1, 'neutral')",
            [],
        )
        .unwrap();
        assert_eq!(weighted_unprocessed_signal_sum(&conn, 1).unwrap(), 2);
    }

    #[test]
    fn growth_due_periodic_updates_anchor() {
        let conn = open_mem();
        let mut last = None;
        let cfg = GrowthScheduleConfig {
            interval_secs: 60,
            weighted_min: 99,
            signal_window: 10,
            sweep_interval_secs: 86_400,
            min_run_gap_secs: 0,
            raw_unprocessed_threshold: 99,
        };
        let t0 = 1_000_000i64;
        assert!(!growth_due(&conn, t0, &mut last, &cfg).unwrap().due);
        assert_eq!(last, Some(t0));

        assert!(!growth_due(&conn, t0 + 30, &mut last, &cfg).unwrap().due);
        let o = growth_due(&conn, t0 + 70, &mut last, &cfg).unwrap();
        assert!(o.due);
        assert!(o.periodic_only);
        assert_eq!(last, Some(t0 + 70));
    }

    #[test]
    fn growth_due_signal_arm_is_not_periodic_only() {
        let conn = open_mem();
        for _ in 0..3 {
            conn.execute(
                "INSERT INTO decisions (evolved, total_tools, failed_tools, feedback)
                 VALUES (0, 2, 0, 'neutral')",
                [],
            )
            .unwrap();
        }
        let mut last = Some(1_000_000i64);
        let cfg = GrowthScheduleConfig {
            interval_secs: 60,
            weighted_min: 3,
            signal_window: 10,
            sweep_interval_secs: 86_400,
            min_run_gap_secs: 0,
            raw_unprocessed_threshold: 99,
        };
        let t = 2_000_000i64;
        let o = growth_due(&conn, t, &mut last, &cfg).unwrap();
        assert!(o.due);
        assert!(!o.periodic_only);
    }

    #[test]
    fn signal_burst_weighted_triggers() {
        let conn = open_mem();
        let cfg = GrowthScheduleConfig {
            interval_secs: 600,
            weighted_min: 3,
            signal_window: 10,
            sweep_interval_secs: 86_400,
            min_run_gap_secs: 0,
            raw_unprocessed_threshold: 99,
        };
        assert!(!signal_burst_due(&conn, &cfg).unwrap());
        for _ in 0..3 {
            conn.execute(
                "INSERT INTO decisions (evolved, total_tools, failed_tools, feedback)
                 VALUES (0, 2, 0, 'neutral')",
                [],
            )
            .unwrap();
        }
        assert!(signal_burst_due(&conn, &cfg).unwrap());
    }

    #[test]
    fn max_ts_material_run_ignores_noop_rows() {
        let conn = open_mem();
        conn.execute(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version)
             VALUES ('2020-01-01T00:00:00Z', 'evolution_run', 'run', 'm', 't1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version)
             VALUES ('2030-01-01T00:00:00Z', 'evolution_run_noop', 'run', 'n', 't2')",
            [],
        )
        .unwrap();
        let max: String = conn
            .query_row(
                "SELECT MAX(ts) FROM evolution_log WHERE type = ?1",
                params![EVOLUTION_LOG_TYPE_RUN_MATERIAL],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            max.starts_with("2020-01-01"),
            "expected older material ts, got {max:?}"
        );
    }
}
