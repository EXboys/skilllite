//! Seccomp BPF Module for Unix Socket Isolation (Linux only)
//!
//! This module implements seccomp-bpf filters to block Unix domain socket
//! creation at the syscall level. This provides an additional layer of
//! security to prevent processes from creating new Unix domain sockets
//! for local IPC (unless explicitly allowed).
//!
//! How it works:
//! - Pre-generated BPF filters intercept the socket() syscall
//! - Blocks creation of AF_UNIX sockets by returning EPERM
//! - Does not affect inherited file descriptors or SCM_RIGHTS
//!
//! Architecture support: x86_64 and aarch64

#![cfg(target_os = "linux")]

use std::io;

// ============================================================================
// Seccomp Constants
// ============================================================================

/// AF_UNIX socket family constant
const AF_UNIX: u32 = 1;

/// Syscall numbers for socket() on different architectures
#[cfg(target_arch = "x86_64")]
const SYS_SOCKET: u32 = 41;

#[cfg(target_arch = "aarch64")]
const SYS_SOCKET: u32 = 198;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const SYS_SOCKET: u32 = 0; // Unsupported architecture

/// Seccomp action: Allow the syscall
const SECCOMP_RET_ALLOW: u32 = 0x7fff0000;

/// Seccomp action: Return errno
const SECCOMP_RET_ERRNO: u32 = 0x00050000;

/// EPERM error code
const EPERM: u32 = 1;

/// Seccomp operation: Set mode filter
const SECCOMP_SET_MODE_FILTER: u32 = 1;

/// PR_SET_NO_NEW_PRIVS
const PR_SET_NO_NEW_PRIVS: i32 = 38;

// ============================================================================
// BPF Filter Structures
// ============================================================================

/// BPF instruction
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

impl SockFilter {
    const fn new(code: u16, jt: u8, jf: u8, k: u32) -> Self {
        Self { code, jt, jf, k }
    }
}

/// BPF program
#[repr(C)]
struct SockFprog {
    len: u16,
    filter: *const SockFilter,
}

// BPF instruction codes
const BPF_LD: u16 = 0x00;
const BPF_W: u16 = 0x00;
const BPF_ABS: u16 = 0x20;
const BPF_JMP: u16 = 0x05;
const BPF_JEQ: u16 = 0x10;
const BPF_K: u16 = 0x00;
const BPF_RET: u16 = 0x06;

// Seccomp data offsets
const SECCOMP_DATA_NR: u32 = 0;        // Syscall number offset
const SECCOMP_DATA_ARGS: u32 = 16;     // Args offset (args[0] is at offset 16)

// ============================================================================
// Unix Socket Filter Configuration
// ============================================================================

/// Configuration for Unix socket blocking
#[derive(Debug, Clone, Default)]
pub struct SeccompConfig {
    /// Whether to block Unix socket creation
    pub block_unix_sockets: bool,
    /// Allowed socket paths (not enforceable via seccomp, for documentation)
    pub allowed_socket_paths: Vec<String>,
}

impl SeccompConfig {
    /// Create a config that blocks all Unix sockets
    pub fn block_all_unix_sockets() -> Self {
        Self {
            block_unix_sockets: true,
            allowed_socket_paths: Vec::new(),
        }
    }

    /// Create a config that allows all Unix sockets
    pub fn allow_all() -> Self {
        Self {
            block_unix_sockets: false,
            allowed_socket_paths: Vec::new(),
        }
    }
}

// ============================================================================
// Seccomp Filter Application
// ============================================================================

/// Apply seccomp filter to block Unix socket creation
///
/// This function must be called in the child process before exec.
/// It sets PR_SET_NO_NEW_PRIVS and applies a BPF filter that blocks
/// socket(AF_UNIX, ...) syscalls.
///
/// # Safety
/// This function uses unsafe syscalls and should only be called
/// in a forked child process before exec.
pub fn apply_unix_socket_filter() -> io::Result<()> {
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Seccomp Unix socket blocking is only supported on x86_64 and aarch64",
        ));
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // First, set NO_NEW_PRIVS to allow unprivileged seccomp
        let ret = unsafe { libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }

        // Build the BPF filter
        let filter = build_unix_socket_filter();

        // Apply the filter
        let prog = SockFprog {
            len: filter.len() as u16,
            filter: filter.as_ptr(),
        };

        let ret = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                SECCOMP_SET_MODE_FILTER as libc::c_ulong,
                0 as libc::c_ulong,
                &prog as *const SockFprog as libc::c_ulong,
            )
        };

        if ret != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

/// Build a BPF filter that blocks socket(AF_UNIX, ...) syscalls
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn build_unix_socket_filter() -> Vec<SockFilter> {
    vec![
        // Load syscall number
        SockFilter::new(BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR),
        
        // If not socket(), allow
        SockFilter::new(BPF_JMP | BPF_JEQ | BPF_K, 0, 3, SYS_SOCKET),
        
        // Load first argument (domain/family)
        SockFilter::new(BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS),
        
        // If AF_UNIX, return EPERM
        SockFilter::new(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, AF_UNIX),
        SockFilter::new(BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ERRNO | EPERM),
        
        // Allow everything else
        SockFilter::new(BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ALLOW),
    ]
}

// ============================================================================
// Pre-exec Hook for Sandbox
// ============================================================================

/// Apply seccomp filter in a pre_exec hook
///
/// This is designed to be used with Command::pre_exec() in the sandbox.
///
/// # Example
/// ```ignore
/// use std::process::Command;
/// use std::os::unix::process::CommandExt;
///
/// let mut cmd = Command::new("python");
/// unsafe {
///     cmd.pre_exec(|| {
///         apply_unix_socket_filter_pre_exec()
///     });
/// }
/// ```
pub fn apply_unix_socket_filter_pre_exec() -> io::Result<()> {
    apply_unix_socket_filter()
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Check if seccomp is supported on this system
pub fn is_seccomp_supported() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // Try to check if seccomp is available
        // We do this by checking if the kernel supports it
        let ret = unsafe {
            libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
        };
        ret == 0
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        false
    }
}

/// Get the current architecture name
pub fn get_architecture() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    { "x86_64" }
    
    #[cfg(target_arch = "aarch64")]
    { "aarch64" }
    
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    { "unsupported" }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seccomp_config() {
        let config = SeccompConfig::block_all_unix_sockets();
        assert!(config.block_unix_sockets);
        assert!(config.allowed_socket_paths.is_empty());

        let config = SeccompConfig::allow_all();
        assert!(!config.block_unix_sockets);
    }

    #[test]
    fn test_architecture_detection() {
        let arch = get_architecture();
        #[cfg(target_arch = "x86_64")]
        assert_eq!(arch, "x86_64");
        
        #[cfg(target_arch = "aarch64")]
        assert_eq!(arch, "aarch64");
    }

    #[test]
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn test_filter_generation() {
        let filter = build_unix_socket_filter();
        assert!(!filter.is_empty());
        // Filter should have 6 instructions
        assert_eq!(filter.len(), 6);
    }

    #[test]
    fn test_seccomp_support_check() {
        // This just checks that the function doesn't panic
        let _ = is_seccomp_supported();
    }
}
