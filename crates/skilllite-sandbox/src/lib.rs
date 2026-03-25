pub mod bash_validator;
pub mod common;
pub mod env;
pub mod log;
pub mod move_protection;
pub mod network_proxy;
pub mod runner;
pub mod runtime_resolver;
pub mod sandbox_backend;
pub mod seatbelt;
pub mod security;

/// 运行时依赖进度回调类型（P0：过程透明）。桌面端可传 `Some(Box::new(|msg| { ... }))` 展示进度。
pub use env::runtime_deps::RuntimeProgressFn;
/// 下载前确认回调与请求类型。传 `Some` 时会在下载 Python/Node 前调用，返回 `false` 则中止。
pub use env::runtime_deps::{
    cli_confirm_download, get_runtime_dir, probe_runtime_for_ui, RuntimeConfirmDownloadFn,
    RuntimeDownloadKind, RuntimeDownloadRequest, RuntimeUiLine, RuntimeUiSnapshot,
};

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub mod seccomp;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;
