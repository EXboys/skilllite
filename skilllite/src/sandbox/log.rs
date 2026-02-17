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
    std::env::var("SKILLLITE_QUIET").or_else(|_| std::env::var("SKILLBOX_QUIET"))
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}
