//! Quiet-mode aware logging. When SKILLLITE_QUIET=1 (e.g. IPC daemon/benchmark), suppress [INFO].

#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {{
        if !crate::sandbox::log::is_quiet() {
            eprintln!($($arg)*);
        }
    }};
}

pub fn is_quiet() -> bool {
    crate::config::ObservabilityConfig::from_env().quiet
}
