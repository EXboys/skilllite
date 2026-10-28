//! Sandbox plans for one subscription run.
//!
//! This module does not call skill `execute` or `execute_with_bwrap`.

use std::net::IpAddr;
use std::path::Path;

use crate::error::bail;
use crate::move_protection::escape_path_for_seatbelt;
use crate::Result;

use super::proxy::ResolvedProxy;

/// Host paths that exist and may be mounted read-only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct HostLayout {
    pub usr: bool,
    pub lib: bool,
    pub bin: bool,
    pub lib64: bool,
    pub ssl_certs: bool,
    pub ca_certificates: bool,
    pub binary_prefix: Option<String>,
}

/// Arguments passed to `bwrap` after the program name, including the child argv.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn bwrap_arguments(
    run_id: &str,
    home_host: &Path,
    work_host: &Path,
    layout: &HostLayout,
    proxy: Option<&ResolvedProxy>,
    argv: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    args.extend(
        [
            "--die-with-parent",
            "--unshare-user",
            "--unshare-pid",
            "--unshare-uts",
            "--unshare-ipc",
        ]
        .into_iter()
        .map(str::to_string),
    );
    args.push("--hostname".to_string());
    args.push(sanitize_hostname(run_id));
    args.push("--uid".to_string());
    args.push("1000".to_string());
    args.push("--gid".to_string());
    args.push("1000".to_string());

    if layout.usr {
        push_ro(&mut args, "/usr", "/usr");
    }
    if layout.lib {
        push_ro(&mut args, "/lib", "/lib");
    }
    if layout.bin {
        push_ro(&mut args, "/bin", "/bin");
    }
    if layout.lib64 {
        push_ro(&mut args, "/lib64", "/lib64");
    }
    if layout.ssl_certs {
        push_ro(&mut args, "/etc/ssl/certs", "/etc/ssl/certs");
    }
    if layout.ca_certificates {
        push_ro(
            &mut args,
            "/etc/ca-certificates.conf",
            "/etc/ca-certificates.conf",
        );
    }
    if let Some(prefix) = layout.binary_prefix.as_deref() {
        if !mounted_prefix(prefix) {
            push_ro(&mut args, prefix, prefix);
        }
    }

    let home = home_host.to_string_lossy().into_owned();
    let work = work_host.to_string_lossy().into_owned();
    args.push("--bind".to_string());
    args.push(home);
    args.push("/home/agent".to_string());
    args.push("--bind".to_string());
    args.push(work);
    args.push("/work".to_string());
    args.push("--tmpfs".to_string());
    args.push("/tmp".to_string());
    args.push("--dev".to_string());
    args.push("/dev".to_string());
    args.push("--proc".to_string());
    args.push("/proc".to_string());
    args.push("--chdir".to_string());
    args.push("/work".to_string());
    args.push("--clearenv".to_string());
    push_env(&mut args, "HOME", "/home/agent");
    push_env(&mut args, "TMPDIR", "/tmp");
    push_env(&mut args, "PATH", "/usr/local/bin:/usr/bin:/bin");
    if let Some(proxy) = proxy {
        for key in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "ALL_PROXY",
            "https_proxy",
            "http_proxy",
            "all_proxy",
        ] {
            push_env(&mut args, key, &proxy.env_url);
        }
        push_env(&mut args, "NO_PROXY", "");
    }
    args.push("--".to_string());
    args.extend(argv.iter().cloned());
    args
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn push_ro(args: &mut Vec<String>, host: &str, dest: &str) {
    args.push("--ro-bind".to_string());
    args.push(host.to_string());
    args.push(dest.to_string());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn push_env(args: &mut Vec<String>, key: &str, value: &str) {
    args.push("--setenv".to_string());
    args.push(key.to_string());
    args.push(value.to_string());
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn mounted_prefix(prefix: &str) -> bool {
    prefix == "/usr"
        || prefix.starts_with("/usr/")
        || prefix == "/bin"
        || prefix.starts_with("/bin/")
        || prefix == "/lib"
        || prefix.starts_with("/lib/")
        || prefix == "/lib64"
        || prefix.starts_with("/lib64/")
}

/// Seatbelt profile for one subscription run.
///
/// macOS `sandbox-exec` accepts only `localhost` or `*` as a network host, so
/// a loopback proxy is written as `localhost:<port>`. A non-loopback address
/// cannot be pinned and is rejected. Reads stay broad because a narrow
/// `subpath` list prevents dyld from starting the process. Writes are limited
/// to the canonical home and work directories.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn seatbelt_profile(home: &Path, work: &Path, proxy: Option<&ResolvedProxy>) -> Result<String> {
    let mut network = String::new();
    if let Some(proxy) = proxy {
        for endpoint in &proxy.endpoints {
            if !endpoint.ip().is_loopback() {
                bail!(
                    "macOS sandbox can only pin a loopback proxy; point https_proxy at 127.0.0.1 or localhost"
                );
            }
            network.push_str(&format!(
                "(allow network-outbound (remote tcp \"localhost:{}\"))\n",
                endpoint.port()
            ));
        }
    } else {
        network.push_str("(allow network-outbound)\n");
    }
    let mut profile = String::from("(version 1)\n(deny default)\n");
    profile.push_str("(allow process*)\n(allow signal)\n(allow sysctl-read)\n");
    profile.push_str("(allow mach*)\n(allow ipc*)\n(allow file-ioctl)\n");
    profile.push_str("(allow file-read*)\n(allow file-map-executable)\n");
    profile.push_str("(allow system-socket)\n");
    profile.push_str("(allow network-inbound (local unix-socket))\n");
    profile.push_str("(allow network-outbound (remote unix-socket))\n");
    profile.push_str(&network);
    for path in [home, work] {
        let escaped = escape_path_for_seatbelt(&path.to_string_lossy());
        profile.push_str(&format!("(allow file-write* (subpath {escaped}))\n"));
    }
    Ok(profile)
}

/// nftables script that accepts only the proxy endpoints from one cgroup.
///
/// `cgroup_path` is the path relative to `/sys/fs/cgroup`. nftables resolves
/// that string as a path, so a leaf name such as `skilllite-run` does not
/// install when the cgroup is nested. `cgroup_level` must be the number of
/// components in `cgroup_path`: a smaller level can install and then fail to
/// match, and this chain's policy is accept.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn nft_script(
    table: &str,
    cgroup_path: &str,
    cgroup_level: usize,
    proxy: &ResolvedProxy,
) -> String {
    let mut rules = String::new();
    for endpoint in &proxy.endpoints {
        let family = match endpoint.ip() {
            IpAddr::V4(_) => "ip",
            IpAddr::V6(_) => "ip6",
        };
        let addr = match endpoint.ip() {
            IpAddr::V4(ip) => ip.to_string(),
            IpAddr::V6(ip) => ip.to_string(),
        };
        rules.push_str(&format!(
            "        socket cgroupv2 level {cgroup_level} \"{cgroup_path}\" {family} daddr {addr} tcp dport {} accept\n",
            endpoint.port()
        ));
    }
    format!(
        "table inet {table} {{\n    chain output {{\n        type filter hook output priority 0; policy accept;\n{rules}        socket cgroupv2 level {cgroup_level} \"{cgroup_path}\" meta l4proto {{ tcp, udp }} drop\n    }}\n}}\n"
    )
}

/// Build the proxy allowlist for `cgroup`, or refuse when the path cannot be named.
///
/// Callers must pass the cgroup directory that the sandboxed process joins.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn proxy_nft_script(table: &str, cgroup: &Path, proxy: &ResolvedProxy) -> Result<String> {
    let path = cgroup_v2_relative(cgroup).ok_or_else(|| {
        crate::Error::validation(
            "proxy enforcement unavailable: cgroup path cannot be used in an nftables rule",
        )
    })?;
    let level = cgroup_v2_level(cgroup).ok_or_else(|| {
        crate::Error::validation(
            "proxy enforcement unavailable: cgroup path cannot be used in an nftables rule",
        )
    })?;
    Ok(nft_script(table, &path, level, proxy))
}

/// Depth of `cgroup` under `/sys/fs/cgroup`. nft `socket cgroupv2 level N` uses this depth.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn cgroup_v2_level(cgroup: &Path) -> Option<usize> {
    cgroup_v2_relative(cgroup).map(|relative| relative.split('/').count())
}

/// `cgroup` relative to `/sys/fs/cgroup`, with `/` separators.
///
/// Returns `None` when `cgroup` is the hierarchy root, sits outside it, or
/// contains a character that would break out of the nftables string.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn cgroup_v2_relative(cgroup: &Path) -> Option<String> {
    let relative = cgroup.strip_prefix("/sys/fs/cgroup").ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => {
                let text = part.to_string_lossy();
                if !nft_cgroup_component(&text) {
                    return None;
                }
                parts.push(text.into_owned());
            }
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn nft_cgroup_component(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('"')
        && !text.contains('\\')
        && !text.contains('\n')
        && !text.contains('\r')
        && !text.contains('{')
        && !text.contains('}')
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn sanitize_hostname(run_id: &str) -> String {
    let mut out: String = run_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    if out.is_empty() || out.chars().all(|c| c == '-') {
        out = "run".to_string();
    }
    out
}

pub fn detect_layout(binary: Option<&Path>) -> HostLayout {
    HostLayout {
        usr: Path::new("/usr").is_dir(),
        lib: Path::new("/lib").is_dir(),
        bin: Path::new("/bin").is_dir(),
        lib64: Path::new("/lib64").is_dir(),
        ssl_certs: Path::new("/etc/ssl/certs").is_dir(),
        ca_certificates: Path::new("/etc/ca-certificates.conf").is_file(),
        binary_prefix: binary.and_then(|path| {
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.to_string_lossy().into_owned())
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::PathBuf;

    fn proxy() -> ResolvedProxy {
        ResolvedProxy {
            endpoints: vec![SocketAddr::from((Ipv4Addr::new(127, 0, 0, 1), 9))],
            env_url: "http://127.0.0.1:9".to_string(),
        }
    }

    #[test]
    fn bwrap_plan_skips_net_namespace_and_skill_unshare_all() {
        let layout = HostLayout {
            usr: true,
            lib: true,
            bin: true,
            lib64: false,
            ssl_certs: true,
            ca_certificates: false,
            binary_prefix: Some("/opt/claude/bin".to_string()),
        };
        let args = bwrap_arguments(
            "run/1",
            Path::new("/tmp/home"),
            Path::new("/srv/app"),
            &layout,
            Some(&proxy()),
            &["claude".to_string(), "-p".to_string(), "修测试".to_string()],
        );
        let joined = args.join(" ");
        assert!(!joined.contains("--unshare-all"));
        assert!(!joined.contains("--unshare-net"));
        assert!(joined.contains("--unshare-user"));
        assert!(joined.contains("--unshare-pid"));
        assert!(joined.contains("--unshare-uts"));
        assert!(joined.contains("--unshare-ipc"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--hostname" && w[1] == "run-1"));
        assert!(args
            .windows(3)
            .any(|w| w[0] == "--ro-bind" && w[1] == "/usr" && w[2] == "/usr"));
        assert!(args
            .windows(3)
            .any(|w| w[0] == "--bind" && w[1] == "/tmp/home" && w[2] == "/home/agent"));
        assert!(args
            .windows(3)
            .any(|w| w[0] == "--bind" && w[1] == "/srv/app" && w[2] == "/work"));
        assert!(args.windows(3).any(|w| {
            w[0] == "--ro-bind" && w[1] == "/opt/claude/bin" && w[2] == "/opt/claude/bin"
        }));
        assert!(!joined.contains("/home/ "));
        assert!(args.iter().any(|arg| arg == "修测试"));
        assert_eq!(args.last().map(String::as_str), Some("修测试"));
    }

    #[test]
    fn seatbelt_pins_loopback_proxy_by_port() {
        let profile = seatbelt_profile(
            Path::new("/tmp/home"),
            Path::new("/srv/app"),
            Some(&proxy()),
        )
        .unwrap();
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(allow network-outbound (remote tcp \"localhost:9\"))"));
        assert!(!profile.contains("(allow network*)"));
        assert!(!profile.contains("(allow network-outbound)\n"));
        assert!(profile.contains("/tmp/home"));
        assert!(profile.contains("/srv/app"));
    }

    #[test]
    fn seatbelt_rejects_a_public_proxy_address() {
        let proxy = ResolvedProxy {
            endpoints: vec![SocketAddr::from((Ipv4Addr::new(1, 1, 1, 1), 443))],
            env_url: "http://1.1.1.1:443".to_string(),
        };
        let err = seatbelt_profile(Path::new("/tmp/home"), Path::new("/tmp/work"), Some(&proxy))
            .unwrap_err();
        assert!(err.to_string().contains("loopback"));
    }

    #[test]
    fn nft_script_names_the_nested_cgroup_path() {
        let cgroup = Path::new(
            "/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/skilllite-run-1",
        );
        let script = proxy_nft_script("skilllite_run", cgroup, &proxy()).unwrap();
        assert!(script.contains("127.0.0.1"));
        assert!(script.contains("tcp dport 9 accept"));
        assert!(script.contains(
            "socket cgroupv2 level 4 \"user.slice/user-1000.slice/user@1000.service/skilllite-run-1\""
        ));
        assert!(
            !script.contains("level 4 \"skilllite-run-1\""),
            "leaf name is not an nftables cgroup path: {script}"
        );
        assert!(script.contains("meta l4proto { tcp, udp } drop"));
        assert!(!script.contains("skuid"));
        assert!(!script.contains("level 1"));
    }

    #[test]
    fn nft_script_rejects_a_cgroup_outside_the_v2_root() {
        let err = proxy_nft_script("skilllite_run", Path::new("/tmp/skilllite-run"), &proxy())
            .unwrap_err();
        assert!(err.to_string().contains("nftables"));
    }

    #[test]
    fn nft_script_rejects_a_quote_in_the_cgroup_path() {
        let err = proxy_nft_script(
            "skilllite_run",
            Path::new("/sys/fs/cgroup/user.slice/bad\"name"),
            &proxy(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("nftables"));
    }

    #[test]
    fn seatbelt_without_a_proxy_allows_outbound_network() {
        let profile =
            seatbelt_profile(Path::new("/tmp/home"), Path::new("/tmp/work"), None).unwrap();
        assert!(profile.contains("(allow network-outbound)\n"));
        assert!(!profile.contains("localhost:"));
    }

    #[test]
    fn bwrap_without_a_proxy_omits_proxy_env() {
        let layout = HostLayout {
            usr: true,
            lib: false,
            bin: true,
            lib64: false,
            ssl_certs: false,
            ca_certificates: false,
            binary_prefix: None,
        };
        let args = bwrap_arguments(
            "run-1",
            Path::new("/tmp/home"),
            Path::new("/tmp/work"),
            &layout,
            None,
            &["/bin/echo".to_string()],
        );
        assert!(!args.iter().any(|arg| arg == "HTTPS_PROXY"));
        assert!(args.iter().any(|arg| arg == "--unshare-user"));
    }

    #[test]
    fn cgroup_level_counts_components_under_the_v2_root() {
        assert_eq!(
            cgroup_v2_level(Path::new(
                "/sys/fs/cgroup/user.slice/app.slice/skilllite-run"
            )),
            Some(3)
        );
        assert_eq!(cgroup_v2_level(Path::new("/sys/fs/cgroup")), None);
        assert_eq!(cgroup_v2_level(Path::new("/tmp/skilllite-run")), None);
    }

    #[test]
    fn hostname_is_bounded() {
        assert_eq!(sanitize_hostname("你好"), "run");
        assert_eq!(sanitize_hostname("Abc_1"), "Abc-1");
        let long = "a".repeat(80);
        assert_eq!(sanitize_hostname(&long).chars().count(), 64);
    }

    #[test]
    fn detect_layout_records_parent_of_binary() {
        let layout = detect_layout(Some(PathBuf::from("/usr/local/bin/claude").as_path()));
        assert_eq!(layout.binary_prefix.as_deref(), Some("/usr/local/bin"));
    }
}
