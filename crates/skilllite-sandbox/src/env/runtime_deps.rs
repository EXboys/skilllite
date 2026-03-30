//! Runtime dependencies: ensure Python/Node are available (prefer system, provision to
//! `~/.skilllite/runtime/` on first use when missing).
//!
//! Strategy: prefer system `python`/`node`; if missing or version below minimum, provision
//! a bundled runtime and report progress via the optional callback (P0: transparent UX).
//!
//! Version policy: system runtimes below minimum (Python 3.10+, Node 18+) are treated as
//! unavailable and we use the bundled runtime. Upgrading the system runtime is left to the
//! user (e.g. via package manager or chat/agent); we do not provide an in-app "upgrade"
//! action for the bundled runtime.
//!
//! ## Security (integrity)
//!
//! Per project policy we **verify hash before extract** (see C-END 5.2: 校验哈希后解压).
//! After download we compute SHA-256 of the archive and compare to the expected value:
//! - **Node**: we fetch `SHASUMS256.txt` from the same Node.js dist and verify the
//!   downloaded tarball.
//! - **Python**: we use a pinned expected SHA-256 per asset (from the official
//!   python-build-standalone release). HTTPS alone is not enough (supply-chain / CDN risk).
//!
//! ## Mainland / international (mirror)
//!
//! We do **not** auto-detect region. Use env vars to override download base URLs when
//! GitHub or nodejs.org is slow or blocked (e.g. mainland China):
//! - `SKILLLITE_RUNTIME_PYTHON_BASE_URL`: default GitHub releases; set to a mirror base
//!   (e.g. `https://mirror.ghproxy.com/https://github.com/astral-sh/python-build-standalone/releases/download`).
//! - `SKILLLITE_RUNTIME_NODE_BASE_URL`: default `https://nodejs.org/dist`; set to a Node
//!   mirror (e.g. `https://npmmirror.com/mirrors/node`).
//!   If the primary URL fails (e.g. connection error), we try one built-in fallback mirror
//!   before giving up.
//!
//! ## Download confirmation
//!
//! Callers can pass a [`RuntimeConfirmDownloadFn`] to `ensure_environment`; if set, it is
//! invoked before downloading Python/Node. Return `false` to abort. When not set, download
//! proceeds without confirmation. For CLI/desktop: pass a callback that prompts the user
//! (and checks `SKILLLITE_AUTO_APPROVE_RUNTIME=1` to skip prompt in CI/automation).

use anyhow::{Context, Result};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Optional progress callback for provisioning (e.g. desktop can show "Preparing Python… 45%").
pub type RuntimeProgressFn = Option<Box<dyn Fn(&str) + Send>>;

/// Kind of runtime that may need to be downloaded.
#[derive(Clone, Debug)]
pub enum RuntimeDownloadKind {
    Python,
    Node,
}

/// Request to confirm a runtime download (CLI/desktop can prompt user).
#[derive(Clone, Debug)]
pub struct RuntimeDownloadRequest {
    pub kind: RuntimeDownloadKind,
    /// Approximate size in MB for UX (e.g. "Download Python (~50 MB)?")
    pub approx_size_mb: u32,
    /// Short label, e.g. "Python 3.12" or "Node.js 20"
    pub label: &'static str,
}

impl RuntimeDownloadRequest {
    pub fn python() -> Self {
        Self {
            kind: RuntimeDownloadKind::Python,
            approx_size_mb: 50,
            label: "Python 3.12",
        }
    }
    pub fn node() -> Self {
        Self {
            kind: RuntimeDownloadKind::Node,
            approx_size_mb: 35,
            label: "Node.js 20",
        }
    }
}

/// Optional callback to confirm before downloading a runtime. If `Some`, called before any
/// download; if it returns `false`, provisioning is aborted. Pass `None` to allow download
/// without confirmation (or use env `SKILLLITE_AUTO_APPROVE_RUNTIME=1` when using a prompt).
pub type RuntimeConfirmDownloadFn = Option<Box<dyn Fn(&RuntimeDownloadRequest) -> bool + Send>>;

/// Build a default CLI confirm callback: prints a prompt to stderr, reads Y/n from stdin.
/// If `SKILLLITE_AUTO_APPROVE_RUNTIME=1` is set, auto-approves without prompting.
pub fn cli_confirm_download() -> RuntimeConfirmDownloadFn {
    Some(Box::new(|req: &RuntimeDownloadRequest| -> bool {
        if std::env::var("SKILLLITE_AUTO_APPROVE_RUNTIME")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            return true;
        }
        let kind_label = match req.kind {
            RuntimeDownloadKind::Python => "Python",
            RuntimeDownloadKind::Node => "Node.js",
        };
        eprintln!();
        eprintln!(
            "This skill requires {} but no compatible version was found on your system.",
            kind_label
        );
        eprintln!(
            "SkillLite needs to download {} (~{} MB) to set up the runtime environment.",
            req.label, req.approx_size_mb
        );
        eprint!("Allow download? [Y/n] ");
        use std::io::Write;
        let _ = std::io::stderr().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        let answer = input.trim().to_lowercase();
        answer.is_empty() || answer == "y" || answer == "yes"
    }))
}

/// Minimum Python version we accept from the system (otherwise use bundled). (3, 10) = 3.10+.
pub const MIN_PYTHON_VERSION: (u32, u32) = (3, 10);
/// Minimum Node version we accept from the system (otherwise use bundled). 18 = v18+.
pub const MIN_NODE_MAJOR: u32 = 18;

// ─── Version pin (single place to bump) ─────────────────────────────────────
// When bumping: 1) Update these constants; 2) Update PYTHON_SHA256_TABLE with hashes
// from the new release (see repo Releases → expanded assets → sha256 under each file).
// Note: .../releases/download/TAG without a filename returns 404; we use the full
// asset URL: .../releases/download/TAG/cpython-...tar.gz

#[cfg(feature = "runtime-deps")]
const PYTHON_RELEASE_TAG: &str = "20260310";
#[cfg(feature = "runtime-deps")]
const PYTHON_CPVERSION: &str = "3.12.13"; // CPython version in asset name
#[cfg(feature = "runtime-deps")]
const NODE_VERSION: &str = "20.18.0";

/// Env key: override Python release download base (e.g. mirror for mainland).
/// Full URL = {base}/{tag}/{archive_name}. No trailing slash.
#[cfg(feature = "runtime-deps")]
const ENV_PYTHON_BASE_URL: &str = "SKILLLITE_RUNTIME_PYTHON_BASE_URL";
/// Env key: override Node dist base (e.g. npmmirror). URL = {base}/v{version}/{file}.
#[cfg(feature = "runtime-deps")]
const ENV_NODE_BASE_URL: &str = "SKILLLITE_RUNTIME_NODE_BASE_URL";

#[cfg(feature = "runtime-deps")]
const DEFAULT_PYTHON_BASE: &str =
    "https://github.com/astral-sh/python-build-standalone/releases/download";
#[cfg(feature = "runtime-deps")]
const FALLBACK_PYTHON_BASE: &str =
    "https://mirror.ghproxy.com/https://github.com/astral-sh/python-build-standalone/releases/download";
#[cfg(feature = "runtime-deps")]
const DEFAULT_NODE_BASE: &str = "https://nodejs.org/dist";
#[cfg(feature = "runtime-deps")]
const FALLBACK_NODE_BASE: &str = "https://npmmirror.com/mirrors/node";

/// Single runtime line for desktop UI (Python / Node): system PATH vs downloaded cache vs not yet provisioned.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUiLine {
    /// `"system"` | `"cache"` | `"none"`
    pub source: String,
    pub label: String,
    /// 在文件管理器中打开/定位用（绝对路径）；`none` 且可解析缓存根时指向 runtime 根目录（可能尚不存在，由桌面端打开时创建）
    pub reveal_path: Option<String>,
    /// 补充说明：例如已用系统解释器但缓存内仍有 SkillLite 下载包时，避免用户误以为「没下载」
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUiSnapshot {
    pub python: RuntimeUiLine,
    pub node: RuntimeUiLine,
    /// e.g. `~/.cache/skilllite/runtime` when known
    pub cache_root: Option<String>,
    /// 缓存根目录绝对路径（用于在访达/资源管理器中打开）
    pub cache_root_abs: Option<String>,
}

/// 桌面端「预下载内置运行时」单条结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRuntimeItem {
    pub requested: bool,
    pub ok: bool,
    pub message: String,
}

/// 一次预下载 Python / Node 的汇总（可只选其一）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRuntimesResult {
    pub python: ProvisionRuntimeItem,
    pub node: ProvisionRuntimeItem,
}

#[derive(Clone)]
struct PythonProbeCandidate {
    program: PathBuf,
    args: Vec<&'static str>,
}

fn python_probe_candidates() -> Vec<PythonProbeCandidate> {
    if cfg!(windows) {
        vec![
            PythonProbeCandidate {
                program: PathBuf::from("python"),
                args: Vec::new(),
            },
            PythonProbeCandidate {
                program: PathBuf::from("py"),
                args: vec!["-3"],
            },
            PythonProbeCandidate {
                program: PathBuf::from("python3"),
                args: Vec::new(),
            },
            PythonProbeCandidate {
                program: PathBuf::from("py"),
                args: Vec::new(),
            },
        ]
    } else {
        vec![
            PythonProbeCandidate {
                program: PathBuf::from("python3"),
                args: Vec::new(),
            },
            PythonProbeCandidate {
                program: PathBuf::from("python"),
                args: Vec::new(),
            },
        ]
    }
}

fn python_cache_binary(runtime_dir: &Path) -> PathBuf {
    let root = runtime_dir.join("python-3.12");
    #[cfg(windows)]
    {
        root.join("python.exe")
    }
    #[cfg(not(windows))]
    {
        root.join("bin").join("python")
    }
}

fn node_cache_binary(runtime_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        runtime_dir.join("node-20").join("node.exe")
    }
    #[cfg(not(windows))]
    {
        runtime_dir.join("node-20").join("bin").join("node")
    }
}

fn display_path_with_tilde(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if path.starts_with(&home) {
            let rest = path.strip_prefix(&home).unwrap_or(path);
            return format!("~{}", rest.to_string_lossy());
        }
    }
    path.to_string_lossy().into_owned()
}

/// 解析当前候选正在使用的 Python 可执行文件绝对路径（含 Windows `py -3` 场景）。
fn resolve_system_python_executable(c: &PythonProbeCandidate) -> Option<PathBuf> {
    let mut cmd = Command::new(&c.program);
    cmd.args(&c.args)
        .args(["-c", "import sys; print(sys.executable)"]);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let line = std::str::from_utf8(&out.stdout)
        .ok()?
        .lines()
        .next()?
        .trim();
    if line.is_empty() {
        return None;
    }
    let p = PathBuf::from(line);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn path_to_reveal_string(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Some(p.to_string_lossy().into_owned())
}

/// Probe where Python / Node will come from (system vs `~/.cache/skilllite/runtime` download), without downloading.
pub fn probe_runtime_for_ui(cache_override: Option<&str>) -> RuntimeUiSnapshot {
    skilllite_core::config::load_dotenv();
    let runtime_dir = get_runtime_dir(cache_override);
    let cache_root = runtime_dir.as_ref().map(|p| display_path_with_tilde(p));

    let python = {
        let mut system_hit: Option<(String, PathBuf)> = None;
        for c in python_probe_candidates() {
            let mut cmd = Command::new(&c.program);
            cmd.args(&c.args).arg("--version");
            let Ok(out) = cmd.output() else {
                continue;
            };
            if !out.status.success() {
                continue;
            }
            let text = std::str::from_utf8(&out.stdout)
                .or_else(|_| std::str::from_utf8(&out.stderr))
                .unwrap_or("");
            if let Some((maj, min)) = parse_python_version(text) {
                if python_version_meets_minimum(maj, min) {
                    let lab = format!("Python {}.{}", maj, min);
                    let exe = resolve_system_python_executable(&c).unwrap_or_else(|| {
                        if c.program.is_absolute() {
                            c.program.clone()
                        } else {
                            which::which(&c.program).unwrap_or_else(|_| c.program.clone())
                        }
                    });
                    system_hit = Some((lab, exe));
                    break;
                }
            }
        }
        if let Some((lab, exe)) = system_hit {
            let skilllite_py_in_cache = runtime_dir
                .as_ref()
                .is_some_and(|rd| python_cache_binary(rd).exists());
            RuntimeUiLine {
                source: "system".into(),
                label: lab,
                reveal_path: path_to_reveal_string(&exe).or_else(|| {
                    exe.is_absolute()
                        .then(|| exe.to_string_lossy().into_owned())
                }),
                detail: skilllite_py_in_cache
                    .then(|| "缓存中已有 SkillLite Python，执行时优先使用系统".into()),
            }
        } else if let Some(ref rd) = runtime_dir {
            let bundle = rd.join("python-3.12");
            if python_cache_binary(rd).exists() {
                RuntimeUiLine {
                    source: "cache".into(),
                    label: "Python（SkillLite 已下载）".into(),
                    reveal_path: path_to_reveal_string(&bundle),
                    detail: None,
                }
            } else {
                RuntimeUiLine {
                    source: "none".into(),
                    label: "首次需要时将自动下载".into(),
                    reveal_path: Some(rd.to_string_lossy().into_owned()),
                    detail: None,
                }
            }
        } else {
            RuntimeUiLine {
                source: "none".into(),
                label: "无法解析缓存目录".into(),
                reveal_path: None,
                detail: None,
            }
        }
    };

    let node = {
        if let Some(path) = which_node() {
            let ver = Command::new(&path)
                .arg("--version")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "v18+".into());
            let skilllite_node_in_cache = runtime_dir
                .as_ref()
                .is_some_and(|rd| node_cache_binary(rd).exists());
            RuntimeUiLine {
                source: "system".into(),
                label: format!("Node {}", ver),
                reveal_path: path_to_reveal_string(&path).or_else(|| {
                    path.is_absolute()
                        .then(|| path.to_string_lossy().into_owned())
                }),
                detail: skilllite_node_in_cache
                    .then(|| "缓存中已有 SkillLite Node，执行时优先使用系统".into()),
            }
        } else if let Some(ref rd) = runtime_dir {
            let bundle = rd.join("node-20");
            if node_cache_binary(rd).exists() {
                RuntimeUiLine {
                    source: "cache".into(),
                    label: "Node（SkillLite 已下载）".into(),
                    reveal_path: path_to_reveal_string(&bundle),
                    detail: None,
                }
            } else {
                RuntimeUiLine {
                    source: "none".into(),
                    label: "首次需要时将自动下载".into(),
                    reveal_path: Some(rd.to_string_lossy().into_owned()),
                    detail: None,
                }
            }
        } else {
            RuntimeUiLine {
                source: "none".into(),
                label: "无法解析缓存目录".into(),
                reveal_path: None,
                detail: None,
            }
        }
    };

    let cache_root_abs = runtime_dir
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned());

    RuntimeUiSnapshot {
        python,
        node,
        cache_root,
        cache_root_abs,
    }
}

/// 将 SkillLite 内置 Python / Node 下载到运行时缓存（与 `ensure_*_runtime` 相同包体）。
///
/// - 不依赖系统是否已安装解释器；适合无 Python/Node 的机器提前下载。
/// - `force == true` 时先删除已有 `python-3.12` / `node-20` 再下载，用于修复损坏缓存。
pub fn provision_runtimes_to_cache(
    cache_override: Option<&str>,
    download_python: bool,
    download_node: bool,
    force: bool,
    #[cfg_attr(not(feature = "runtime-deps"), allow(unused_variables))]
    python_progress: RuntimeProgressFn,
    #[cfg_attr(not(feature = "runtime-deps"), allow(unused_variables))]
    node_progress: RuntimeProgressFn,
) -> ProvisionRuntimesResult {
    skilllite_core::config::load_dotenv();
    let Some(runtime_dir) = get_runtime_dir(cache_override) else {
        let fail = ProvisionRuntimeItem {
            requested: true,
            ok: false,
            message: "无法解析运行时缓存目录".into(),
        };
        return ProvisionRuntimesResult {
            python: if download_python {
                fail.clone()
            } else {
                ProvisionRuntimeItem {
                    requested: false,
                    ok: true,
                    message: "未选择下载 Python".into(),
                }
            },
            node: if download_node {
                fail
            } else {
                ProvisionRuntimeItem {
                    requested: false,
                    ok: true,
                    message: "未选择下载 Node".into(),
                }
            },
        };
    };

    let python = if !download_python {
        drop(python_progress);
        ProvisionRuntimeItem {
            requested: false,
            ok: true,
            message: "未选择下载 Python".into(),
        }
    } else {
        #[cfg(feature = "runtime-deps")]
        {
            let bin = python_cache_binary(&runtime_dir);
            let dir = runtime_dir.join("python-3.12");
            if bin.exists() && !force {
                drop(python_progress);
                ProvisionRuntimeItem {
                    requested: true,
                    ok: true,
                    message: "Python 已在缓存中".into(),
                }
            } else {
                let py_result: Result<(), String> = if force && dir.exists() {
                    std::fs::remove_dir_all(&dir)
                        .map_err(|e| format!("无法删除旧 Python 缓存：{}", e))
                        .and_then(|_| {
                            ensure_python_runtime(&runtime_dir, python_progress)
                                .map(|_| ())
                                .map_err(|e| e.to_string())
                        })
                } else {
                    ensure_python_runtime(&runtime_dir, python_progress)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                };
                match py_result {
                    Ok(()) => ProvisionRuntimeItem {
                        requested: true,
                        ok: true,
                        message: if force {
                            "已重新下载 Python 运行时".into()
                        } else {
                            "已下载 Python 运行时".into()
                        },
                    },
                    Err(msg) => ProvisionRuntimeItem {
                        requested: true,
                        ok: false,
                        message: msg,
                    },
                }
            }
        }
        #[cfg(not(feature = "runtime-deps"))]
        {
            drop(python_progress);
            ProvisionRuntimeItem {
                requested: true,
                ok: false,
                message: "当前构建未启用内置运行时下载（runtime-deps）".into(),
            }
        }
    };

    let node = if !download_node {
        drop(node_progress);
        ProvisionRuntimeItem {
            requested: false,
            ok: true,
            message: "未选择下载 Node".into(),
        }
    } else {
        #[cfg(feature = "runtime-deps")]
        {
            let bin = node_cache_binary(&runtime_dir);
            let dir = runtime_dir.join("node-20");
            if bin.exists() && !force {
                drop(node_progress);
                ProvisionRuntimeItem {
                    requested: true,
                    ok: true,
                    message: "Node 已在缓存中".into(),
                }
            } else {
                let node_result = if force && dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&dir) {
                        Err(anyhow::anyhow!("无法删除旧 Node 缓存：{}", e))
                    } else {
                        ensure_node_runtime(&runtime_dir, node_progress).map(|_| ())
                    }
                } else {
                    ensure_node_runtime(&runtime_dir, node_progress).map(|_| ())
                };
                match node_result {
                    Ok(()) => ProvisionRuntimeItem {
                        requested: true,
                        ok: true,
                        message: if force {
                            "已重新下载 Node 运行时".into()
                        } else {
                            "已下载 Node 运行时".into()
                        },
                    },
                    Err(e) => ProvisionRuntimeItem {
                        requested: true,
                        ok: false,
                        message: format!("{}", e),
                    },
                }
            }
        }
        #[cfg(not(feature = "runtime-deps"))]
        {
            drop(node_progress);
            ProvisionRuntimeItem {
                requested: true,
                ok: false,
                message: "当前构建未启用内置运行时下载（runtime-deps）".into(),
            }
        }
    };

    ProvisionRuntimesResult { python, node }
}

/// Returns the runtime root directory (same base as cache, subdir `runtime`).
/// E.g. cache_dir = ~/.cache/skilllite/envs => runtime = ~/.cache/skilllite/runtime.
pub fn get_runtime_dir(override_cache_dir: Option<&str>) -> Option<PathBuf> {
    let base = override_cache_dir
        .map(PathBuf::from)
        .or_else(|| {
            skilllite_core::config::load_dotenv();
            skilllite_core::config::CacheConfig::cache_dir().map(PathBuf::from)
        })
        .or_else(|| dirs::cache_dir().map(|d| d.join("skilllite")))?;
    // Config cache_dir may be "base" or "base/envs"; normalize to base then add runtime
    let base = if base.file_name().map(|n| n == "envs").unwrap_or(false) {
        base.parent().map(PathBuf::from).unwrap_or(base)
    } else {
        base
    };
    Some(base.join("runtime"))
}

/// Returns system Node path if present and version >= MIN_NODE_MAJOR; otherwise None.
pub fn which_node() -> Option<PathBuf> {
    let path = which::which("node").ok()?;
    let out = Command::new(&path).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v = parse_node_version(std::str::from_utf8(&out.stdout).ok()?)?;
    if v >= MIN_NODE_MAJOR {
        Some(path)
    } else {
        None
    }
}

/// Returns system npm path if present (and node is usable); otherwise None.
pub fn which_npm() -> Option<PathBuf> {
    which_node()?;
    let out = Command::new("npm").arg("--version").output().ok()?;
    if out.status.success() {
        which::which("npm").ok()
    } else {
        None
    }
}

/// Parse "v18.0.0" or "v20.10.0" -> major version.
fn parse_node_version(s: &str) -> Option<u32> {
    let s = s.trim().strip_prefix('v')?.trim();
    let major = s.split('.').next()?.parse::<u32>().ok()?;
    Some(major)
}

/// Parses Python version from stdout of `python --version` (e.g. "Python 3.10.2" or "Python 3.12.0").
/// Returns (major, minor) or None if unparseable.
pub fn parse_python_version(stdout: &str) -> Option<(u32, u32)> {
    let s = stdout.trim();
    let after = s.strip_prefix("Python ")?.trim();
    let mut parts = after.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor))
}

/// Returns true if (major, minor) >= MIN_PYTHON_VERSION.
pub fn python_version_meets_minimum(major: u32, minor: u32) -> bool {
    let (min_maj, min_min) = MIN_PYTHON_VERSION;
    major > min_maj || (major == min_maj && minor >= min_min)
}

#[cfg(feature = "runtime-deps")]
pub fn ensure_python_runtime(runtime_dir: &Path, progress: RuntimeProgressFn) -> Result<PathBuf> {
    let report = |msg: &str| {
        if let Some(ref f) = progress {
            f(msg);
        } else {
            eprintln!("[runtime] {}", msg);
        }
        tracing::info!("[runtime] {}", msg);
    };

    let (_triple, archive_name) = python_asset_for_target()?;
    let python_dir = runtime_dir.join("python-3.12");
    let python_bin = python_dir.join("bin").join("python");
    #[cfg(windows)]
    let python_bin = python_dir.join("python.exe");
    if python_bin.exists() {
        return Ok(python_bin);
    }

    report(
        "This skill requires Python but none was found on your system. Preparing automatically...",
    );
    std::fs::create_dir_all(runtime_dir).context("Create runtime dir")?;
    // Remove stale dirs from previous failed runs so we have a clean extract and a single cpython-* candidate
    if python_dir.exists() {
        let _ = std::fs::remove_dir_all(&python_dir);
    }
    for e in std::fs::read_dir(runtime_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("cpython-") && path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    let archive_path = runtime_dir.join(&archive_name);

    let (primary_base, fallback_base) = python_download_bases();
    let url_primary = format!(
        "{}/{}/{}",
        primary_base.trim_end_matches('/'),
        PYTHON_RELEASE_TAG,
        archive_name
    );
    report("Downloading Python runtime...");
    let download_ok = download_with_progress(&url_primary, &archive_path, &progress).is_ok();
    if !download_ok {
        tracing::info!("[runtime] Primary Python URL failed, trying fallback mirror");
        if let Some(ref f) = progress {
            f("Primary source unreachable, trying fallback mirror...");
        } else {
            eprintln!("[runtime] Primary source unreachable, trying fallback mirror...");
        }
        let url_fallback = format!(
            "{}/{}/{}",
            fallback_base.trim_end_matches('/'),
            PYTHON_RELEASE_TAG,
            archive_name
        );
        download_with_progress(&url_fallback, &archive_path, &progress)?;
    }
    if let Some(expected) = expected_python_sha256(&archive_name) {
        report("Verifying integrity...");
        verify_sha256(&archive_path, expected)?;
    } else {
        let _ = std::fs::remove_file(&archive_path);
        anyhow::bail!(
            "Integrity check not configured for Python asset '{}'. Refusing to run unverified runtime.",
            archive_name
        );
    }
    report("Extracting...");
    extract_tar_gz(&archive_path, runtime_dir)?;
    std::fs::remove_file(&archive_path).ok();
    // Top-level dir: official install_only uses "python" (python/install/* rewritten to python/*);
    // older or alternate builds may use "cpython-{version}+{tag}-{triple}-install_only"
    let extracted_root = std::fs::read_dir(runtime_dir)
        .context("Read runtime dir")?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .find(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name == "python" || name.starts_with("cpython-")
        })
        .map(|e| e.path())
        .context("No python/ or cpython-* dir after extract")?;
    // Prefer bin/python (or python.exe) at root; some archives have install/ as the actual tree
    let install_root = if extracted_root.join("bin").join("python").exists()
        || extracted_root.join("python.exe").exists()
    {
        extracted_root.clone()
    } else if extracted_root
        .join("install")
        .join("bin")
        .join("python")
        .exists()
        || extracted_root.join("install").join("python.exe").exists()
    {
        extracted_root.join("install")
    } else {
        anyhow::bail!(
            "Python binary not found under {} (expected bin/python or install/bin/python)",
            extracted_root.display()
        );
    };
    if install_root.join("bin").join("python").exists() || install_root.join("python.exe").exists()
    {
        // Remove stale target dir from a previous failed run so rename can succeed
        if python_dir.exists() {
            std::fs::remove_dir_all(&python_dir).context("Remove stale python dir for rename")?;
        }
        std::fs::rename(&install_root, &python_dir).context("Rename extracted Python dir")?;
    }
    report("Python runtime is ready.");
    if python_bin.exists() {
        Ok(python_bin)
    } else {
        anyhow::bail!("Python runtime not found at {}", python_bin.display())
    }
}

#[cfg(feature = "runtime-deps")]
fn python_download_bases() -> (String, String) {
    skilllite_core::config::load_dotenv();
    let primary =
        std::env::var(ENV_PYTHON_BASE_URL).unwrap_or_else(|_| DEFAULT_PYTHON_BASE.to_string());
    (primary, FALLBACK_PYTHON_BASE.to_string())
}

#[cfg(feature = "runtime-deps")]
fn node_download_bases() -> (String, String) {
    skilllite_core::config::load_dotenv();
    let primary =
        std::env::var(ENV_NODE_BASE_URL).unwrap_or_else(|_| DEFAULT_NODE_BASE.to_string());
    (primary, FALLBACK_NODE_BASE.to_string())
}

#[cfg(feature = "runtime-deps")]
fn python_asset_for_target() -> Result<(String, String)> {
    let (triple, _) = target_triple()?;
    let archive = format!(
        "cpython-{}+{}-{}-install_only.tar.gz",
        PYTHON_CPVERSION, PYTHON_RELEASE_TAG, triple
    );
    Ok((triple.to_string(), archive))
}

/// Expected SHA-256 per asset for the release defined by PYTHON_RELEASE_TAG.
/// When bumping: copy hashes from https://github.com/astral-sh/python-build-standalone/releases (expand assets for the tag).
#[cfg(feature = "runtime-deps")]
fn expected_python_sha256(archive_name: &str) -> Option<&'static str> {
    let table: &[(&str, &str)] = &[
        (
            "cpython-3.12.13+20260310-aarch64-apple-darwin-install_only.tar.gz",
            "58038f6643b0c51385aa8af1549d2f6d9598c7a48383b9c74fc65481b2b5e6d5",
        ),
        (
            "cpython-3.12.13+20260310-x86_64-apple-darwin-install_only.tar.gz",
            "09d7bfb7e2684d746e2d44bd800becfd07c4c672de907340d279409a8bca2d8b",
        ),
        (
            "cpython-3.12.13+20260310-x86_64-unknown-linux-gnu-install_only.tar.gz",
            "eddc8bf40c7fca5032acd5de4b89e748e17b16cf61918320a0506c7e450a8df3",
        ),
        (
            "cpython-3.12.13+20260310-aarch64-unknown-linux-gnu-install_only.tar.gz",
            "872bb2d9959bbcba411af08fa57423d586b779c21c7de70890f99e1c59036efc",
        ),
        (
            "cpython-3.12.13+20260310-armv7-unknown-linux-gnueabihf-install_only.tar.gz",
            "e91a619830b48cd9ff1c917a3eab6d521ea344a6b730182eeeb6895009659486",
        ),
    ];
    table
        .iter()
        .find(|(name, _)| *name == archive_name)
        .map(|(_, hash)| *hash)
}

#[cfg(feature = "runtime-deps")]
fn target_triple() -> Result<(String, String)> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let triple = match (os, arch) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "arm") => "armv7-unknown-linux-gnueabihf",
        _ => anyhow::bail!("Unsupported platform for bundled Python: {}-{}", os, arch),
    };
    let node_suffix = match (os, arch) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") => "linux-armv7l",
        _ => anyhow::bail!("Unsupported platform for bundled Node: {}-{}", os, arch),
    };
    Ok((triple.to_string(), node_suffix.to_string()))
}

/// Verify archive integrity with SHA-256 (per project security policy: verify before extract).
#[cfg(feature = "runtime-deps")]
fn verify_sha256(archive_path: &Path, expected_hex: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(archive_path).context("Open archive for verification")?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf).context("Read archive for verification")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex::encode(hasher.finalize());
    let expected = expected_hex.trim().to_lowercase();
    if got != expected {
        let _ = std::fs::remove_file(archive_path);
        anyhow::bail!(
            "Runtime archive integrity check failed: expected sha256 {} got {}. \
            If using a mirror, try another or set SKILLLITE_RUNTIME_PYTHON_BASE_URL / SKILLLITE_RUNTIME_NODE_BASE_URL.",
            expected_hex,
            got
        );
    }
    Ok(())
}

/// Build a ureq agent with connect and read timeouts for runtime downloads.
#[cfg(feature = "runtime-deps")]
fn runtime_http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(30))
        .timeout_read(std::time::Duration::from_secs(300))
        .build()
}

/// Fetch SHASUMS256.txt and return the expected SHA-256 hex for the given archive filename.
#[cfg(feature = "runtime-deps")]
fn fetch_node_sha256(sums_url: &str, archive_name: &str) -> Result<String> {
    let resp = runtime_http_agent()
        .get(sums_url)
        .call()
        .context("Fetch Node SHASUMS256.txt")?;
    let body = resp.into_string().context("Read SHASUMS256 body")?;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let hash = parts.next().context("Missing hash in SHASUMS256 line")?;
        let name = parts
            .next()
            .context("Missing filename in SHASUMS256 line")?
            .trim();
        if name == archive_name {
            return Ok(hash.to_lowercase());
        }
    }
    anyhow::bail!(
        "Archive '{}' not found in SHASUMS256.txt from {}",
        archive_name,
        sums_url
    );
}

#[cfg(feature = "runtime-deps")]
fn download_with_progress(url: &str, path: &Path, progress: &RuntimeProgressFn) -> Result<()> {
    let resp = runtime_http_agent()
        .get(url)
        .call()
        .context("Download request")?;
    let len: usize = resp
        .header("Content-Length")
        .and_then(|h| h.parse().ok())
        .unwrap_or(0);
    let mut file = std::fs::File::create(path).context("Create archive file")?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 8192];
    let mut total: usize = 0;
    let mut last_report_total: usize = 0;
    // 每约 512KiB 或结束时汇报一次，避免 UI 卡顿又保留可读进度
    const REPORT_STRIDE: usize = 512 * 1024;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).context("Read download body")?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        total += n;
        if len > 0 && total.saturating_sub(last_report_total) >= REPORT_STRIDE {
            last_report_total = total;
            let pct = (total as f64 / len as f64 * 100.0).min(100.0) as u32;
            let msg = format!("Downloading... {}%", pct);
            if let Some(ref f) = progress {
                f(&msg);
            } else {
                eprintln!("[runtime] {}", msg);
            }
        }
    }
    if len > 0 && total > last_report_total {
        let pct = (total as f64 / len as f64 * 100.0).min(100.0) as u32;
        let msg = format!("Downloading... {}%", pct);
        if let Some(ref f) = progress {
            f(&msg);
        } else {
            eprintln!("[runtime] {}", msg);
        }
    }
    Ok(())
}

#[cfg(feature = "runtime-deps")]
fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive_path).context("Open archive")?;
    let dec = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(dec);
    archive.unpack(dest).context("Unpack tar.gz")?;
    Ok(())
}

#[cfg(feature = "runtime-deps")]
pub fn ensure_node_runtime(
    runtime_dir: &Path,
    progress: RuntimeProgressFn,
) -> Result<(PathBuf, PathBuf)> {
    let report = |msg: &str| {
        if let Some(ref f) = progress {
            f(msg);
        } else {
            eprintln!("[runtime] {}", msg);
        }
        tracing::info!("[runtime] {}", msg);
    };

    let (_, node_suffix) = target_triple()?;
    let node_dir = runtime_dir.join("node-20");
    let node_bin = node_dir.join("bin").join("node");
    #[cfg(windows)]
    let node_bin = node_dir.join("node.exe");
    if node_bin.exists() {
        let npm_bin = node_dir.join("bin").join("npm");
        #[cfg(windows)]
        let npm_bin = node_dir.join("npm.cmd");
        return Ok((node_bin, npm_bin));
    }

    report(
        "This skill requires Node.js but none was found on your system. Preparing automatically...",
    );
    std::fs::create_dir_all(runtime_dir).context("Create runtime dir")?;
    // Remove stale dirs from previous failed runs (same as Python)
    if node_dir.exists() {
        let _ = std::fs::remove_dir_all(&node_dir);
    }
    let node_prefix = format!("node-v{}-", NODE_VERSION);
    for e in std::fs::read_dir(runtime_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(&node_prefix) && path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    let archive_name = format!("node-v{}-{}.tar.gz", NODE_VERSION, node_suffix);
    let (primary_base, fallback_base) = node_download_bases();
    let base = primary_base.trim_end_matches('/');
    let url_primary = format!("{}/v{}/{}", base, NODE_VERSION, archive_name);
    let sums_url_primary = format!("{}/v{}/SHASUMS256.txt", base, NODE_VERSION);
    let archive_path = runtime_dir.join(&archive_name);
    report("Downloading Node.js runtime...");
    let primary_ok = download_with_progress(&url_primary, &archive_path, &progress).is_ok();
    let sums_url_used = if primary_ok {
        sums_url_primary
    } else {
        tracing::info!("[runtime] Primary Node URL failed, trying fallback mirror");
        if let Some(ref f) = progress {
            f("Primary source unreachable, trying fallback mirror...");
        } else {
            eprintln!("[runtime] Primary source unreachable, trying fallback mirror...");
        }
        let fallback = fallback_base.trim_end_matches('/');
        let url_fb = format!("{}/v{}/{}", fallback, NODE_VERSION, archive_name);
        download_with_progress(&url_fb, &archive_path, &progress)?;
        format!("{}/v{}/SHASUMS256.txt", fallback, NODE_VERSION)
    };
    report("Verifying integrity...");
    let expected = fetch_node_sha256(&sums_url_used, &archive_name)?;
    verify_sha256(&archive_path, &expected)?;
    report("Extracting...");
    extract_tar_gz(&archive_path, runtime_dir)?;
    std::fs::remove_file(&archive_path).ok();
    let extracted = runtime_dir.join(format!("node-v{}-{}", NODE_VERSION, node_suffix));
    if extracted.exists() {
        if node_dir.exists() {
            std::fs::remove_dir_all(&node_dir).context("Remove stale node dir for rename")?;
        }
        std::fs::rename(&extracted, &node_dir).context("Rename extracted Node dir")?;
    }
    report("Node.js runtime is ready.");
    let npm_bin = node_dir.join("bin").join("npm");
    #[cfg(windows)]
    let npm_bin = node_dir.join("npm.cmd");
    Ok((node_bin, npm_bin))
}

#[cfg(not(feature = "runtime-deps"))]
pub fn ensure_python_runtime(_runtime_dir: &Path, _progress: RuntimeProgressFn) -> Result<PathBuf> {
    anyhow::bail!(
        "Python not found or version < {}.{}. Install Python {}.{}+ or enable runtime-deps feature.",
        MIN_PYTHON_VERSION.0,
        MIN_PYTHON_VERSION.1,
        MIN_PYTHON_VERSION.0,
        MIN_PYTHON_VERSION.1
    )
}

#[cfg(not(feature = "runtime-deps"))]
pub fn ensure_node_runtime(
    _runtime_dir: &Path,
    _progress: RuntimeProgressFn,
) -> Result<(PathBuf, PathBuf)> {
    anyhow::bail!(
        "Node.js not found or version < {}. Install Node {}+ or enable runtime-deps feature.",
        MIN_NODE_MAJOR,
        MIN_NODE_MAJOR
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_version() {
        assert_eq!(parse_python_version("Python 3.10.2"), Some((3, 10)));
        assert_eq!(parse_python_version("Python 3.12.0\n"), Some((3, 12)));
        assert_eq!(parse_python_version("Python 3.9.5"), Some((3, 9)));
        assert_eq!(parse_python_version("invalid"), None);
    }

    #[test]
    fn test_python_version_meets_minimum() {
        assert!(python_version_meets_minimum(3, 10));
        assert!(python_version_meets_minimum(3, 12));
        assert!(python_version_meets_minimum(4, 0));
        assert!(!python_version_meets_minimum(3, 9));
        assert!(!python_version_meets_minimum(2, 7));
    }

    #[test]
    fn test_parse_node_version() {
        assert_eq!(parse_node_version("v18.0.0"), Some(18));
        assert_eq!(parse_node_version("v20.10.0"), Some(20));
        assert_eq!(parse_node_version("  v20.10.0  "), Some(20));
        assert_eq!(parse_node_version("18.0.0"), None);
    }
}
