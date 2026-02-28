//! Quiet-mode aware logging. When SKILLLITE_QUIET=1 (e.g. IPC daemon/benchmark), suppress [INFO].
//! Uses `tracing::info!` so output is captured by the tracing subscriber.

#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {{
        if !$crate::log::is_quiet() {
            tracing::info!($($arg)*);
        }
    }};
}

pub fn is_quiet() -> bool {
    skilllite_core::config::ObservabilityConfig::from_env().quiet
}
