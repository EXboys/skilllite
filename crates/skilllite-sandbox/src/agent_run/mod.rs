//! One sandboxed process for a subscription directory.
//!
//! Skill `execute` does not call this module. The skill path keeps its own
//! network namespace, timeout, and seccomp filter.

mod plan;
mod proxy;
mod redact;

use std::fs::{self, File};
#[cfg(target_os = "linux")]
use std::io::Write;
use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::error::bail;
use crate::Result;

pub use redact::redact_line;

/// Which agent login directory and default command a run uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentRuntime {
    /// Copy `account_dir/.claude`. Default command is Claude Code.
    #[default]
    Claude,
    /// Copy `account_dir/.codex`. Default command is Codex CLI.
    /// After the process exits, write `auth.json` back when it is a regular file.
    Codex,
}

impl AgentRuntime {
    /// Parse the CLI value `claude` or `codex`.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => bail!("runtime must be claude or codex"),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn config_dir(self) -> &'static str {
        match self {
            Self::Claude => ".claude",
            Self::Codex => ".codex",
        }
    }
}

/// Inputs for one subscription sandbox.
///
/// An empty `argv` uses the runtime default command. A non-empty `argv`
/// replaces that command and still copies the runtime's login directory.
#[derive(Debug, Clone)]
pub struct RunRequest {
    pub run_id: String,
    pub account_dir: PathBuf,
    pub work_dir: PathBuf,
    pub prompt: String,
    pub https_proxy: String,
    /// When true, an empty `https_proxy` refuses to start. A provided proxy is
    /// still resolved and enforced when this is false.
    pub require_proxy: bool,
    pub runtime: AgentRuntime,
    pub argv: Vec<String>,
    pub cancel: Option<Arc<AtomicBool>>,
}

impl RunRequest {
    pub fn new(
        account_dir: impl Into<PathBuf>,
        work_dir: impl Into<PathBuf>,
        prompt: impl Into<String>,
        https_proxy: impl Into<String>,
    ) -> Self {
        Self {
            run_id: String::new(),
            account_dir: account_dir.into(),
            work_dir: work_dir.into(),
            prompt: prompt.into(),
            https_proxy: https_proxy.into(),
            require_proxy: false,
            runtime: AgentRuntime::Claude,
            argv: Vec::new(),
            cancel: None,
        }
    }
}

/// Result of a finished sandbox. `stderr_excerpt` is redacted and truncated
/// on a character boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub exit_code: i32,
    pub stderr_excerpt: String,
}

/// Run one sandboxed command.
///
/// Each stdout line is redacted, then passed to `on_line`. The ephemeral home
/// is removed before this function returns. The subscription directory and
/// the work directory are not deleted.
pub fn run(request: RunRequest, mut on_line: impl FnMut(&str)) -> Result<RunOutcome> {
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (request, &mut on_line);
        bail!("agent sandbox run supports macOS and Linux only");
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    run_unix(request, &mut on_line)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_unix(request: RunRequest, on_line: &mut dyn FnMut(&str)) -> Result<RunOutcome> {
    if !request.account_dir.is_dir() {
        bail!("account_dir is not a directory");
    }
    if !request.work_dir.is_dir() {
        bail!("work_dir is not a directory");
    }
    let config_name = request.runtime.config_dir();
    let config_dir = request.account_dir.join(config_name);
    if !config_dir.is_dir() {
        bail!("account_dir has no {config_name} directory");
    }
    let resolved = optional_proxy(&request)?;
    let mut argv = if request.argv.is_empty() {
        default_argv(request.runtime, &request.prompt)
    } else {
        request.argv.clone()
    };
    let (exec_binary, launcher_dir) = canonical_command(Path::new(&argv[0]))?;
    argv[0] = path_to_string(&exec_binary)?;
    let mut layout = plan::detect_layout(Some(exec_binary.as_path()));
    layout.launcher_prefix = launcher_dir;
    let _lock = try_lock_directory(&request.account_dir)?;
    let home = tempfile::TempDir::new()?;
    copy_tree(&config_dir, &home.path().join(config_name))?;
    let home_path = fs::canonicalize(home.path())?;
    let work_path = fs::canonicalize(&request.work_dir)?;
    let run_id = if request.run_id.is_empty() {
        generated_run_id()
    } else {
        request.run_id.clone()
    };
    #[cfg(target_os = "linux")]
    let filter = match &resolved {
        Some(proxy) => Some(install_linux_filter(&run_id, proxy)?),
        None => None,
    };

    let mut child = spawn_sandboxed(
        &run_id,
        &home_path,
        &work_path,
        &layout,
        resolved.as_ref(),
        &argv,
        #[cfg(target_os = "linux")]
        filter.as_ref().map(|item| item.cgroup()),
    )?;
    let pid = child.id() as i32;
    let mut group = ProcessGroup::new(pid);
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(cancel) = request.cancel.clone() {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    signal_group(pid);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
        });
    }

    let stderr_handle = child.stderr.take().map(|stderr| {
        thread::spawn(move || {
            let mut acc = String::new();
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if !acc.is_empty() {
                            acc.push('\n');
                        }
                        acc.push_str(&redact_line(&text));
                    }
                    Err(_) => break,
                }
            }
            acc
        })
    });
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(text) => on_line(&redact_line(&text)),
                Err(err) => {
                    stop.store(true, Ordering::Relaxed);
                    signal_group(pid);
                    let _ = child.wait();
                    group.disarm();
                    persist_runtime_auth(request.runtime, &home_path, &request.account_dir)?;
                    return Err(err.into());
                }
            }
        }
    }
    let status = child.wait();
    group.disarm();
    stop.store(true, Ordering::Relaxed);
    persist_runtime_auth(request.runtime, &home_path, &request.account_dir)?;
    let status = status?;
    let stderr_excerpt = stderr_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr_excerpt = stderr_excerpt.chars().take(2_000).collect();
    Ok(RunOutcome {
        exit_code: status.code().unwrap_or(1),
        stderr_excerpt,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn spawn_sandboxed(
    run_id: &str,
    home: &Path,
    work: &Path,
    layout: &plan::HostLayout,
    proxy: Option<&proxy::ResolvedProxy>,
    argv: &[String],
    #[cfg(target_os = "linux")] cgroup: Option<&Path>,
) -> Result<std::process::Child> {
    if argv.is_empty() {
        bail!("argv is empty");
    }
    #[cfg(target_os = "macos")]
    {
        let _ = run_id;
        let profile = plan::seatbelt_profile(home, work, proxy)?;
        let profile_path = home.join("sandbox.sb");
        fs::write(&profile_path, profile)?;
        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f").arg(&profile_path);
        cmd.arg(&argv[0]);
        cmd.args(&argv[1..]);
        cmd.current_dir(work);
        apply_env(
            &mut cmd,
            home,
            proxy,
            &argv[0],
            layout.launcher_prefix.as_deref(),
        );
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        arm_process_group(&mut cmd);
        Ok(cmd.spawn()?)
    }
    #[cfg(target_os = "linux")]
    {
        let bwrap = which_first(Path::new("bwrap"))
            .ok_or_else(|| crate::Error::validation("bwrap is required for agent sandbox run"))?;
        let args = plan::bwrap_arguments(run_id, home, work, layout, proxy, argv);
        let mut cmd = Command::new(bwrap);
        cmd.args(&args);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cgroup) = cgroup {
            arm_linux_child(&mut cmd, cgroup.join("cgroup.procs"));
        } else {
            arm_process_group(&mut cmd);
        }
        Ok(cmd.spawn()?)
    }
}

#[cfg(target_os = "macos")]
fn apply_env(
    cmd: &mut Command,
    home: &Path,
    proxy: Option<&proxy::ResolvedProxy>,
    argv0: &str,
    launcher_dir: Option<&str>,
) {
    cmd.env_clear();
    cmd.env("HOME", home);
    cmd.env("TMPDIR", home.join("tmp"));
    let _ = fs::create_dir_all(home.join("tmp"));
    cmd.env("PATH", plan::sandbox_path(argv0, launcher_dir, true));
    if let Some(proxy) = proxy {
        for key in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "ALL_PROXY",
            "https_proxy",
            "http_proxy",
            "all_proxy",
        ] {
            cmd.env(key, &proxy.env_url);
        }
        cmd.env("NO_PROXY", "");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn optional_proxy(request: &RunRequest) -> Result<Option<proxy::ResolvedProxy>> {
    if request.https_proxy.trim().is_empty() {
        if request.require_proxy {
            bail!("https_proxy is required");
        }
        return Ok(None);
    }
    Ok(Some(proxy::resolve_proxy(&request.https_proxy)?))
}

#[cfg(unix)]
fn arm_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct ProcessGroup {
    pid: i32,
    armed: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl ProcessGroup {
    fn new(pid: i32) -> Self {
        Self { pid, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if self.armed {
            signal_group(self.pid);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn signal_group(pid: i32) {
    unsafe {
        libc::killpg(pid, libc::SIGKILL);
    }
}

/// Exclusive non-blocking lock on the subscription directory.
/// Dropping the guard releases the lock. The directory contents are not modified.
#[cfg(unix)]
#[derive(Debug)]
struct DirLock {
    _file: File,
}

#[cfg(unix)]
fn try_lock_directory(dir: &Path) -> Result<DirLock> {
    let file = File::open(dir)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            bail!("subscription directory is busy");
        }
        return Err(err.into());
    }
    Ok(DirLock { _file: file })
}

#[cfg(unix)]
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn generated_run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("run{nanos:x}")
}

fn resolve_host_binary(binary: &Path) -> Result<PathBuf> {
    let found = if binary.components().count() > 1 {
        if binary.is_absolute() {
            binary.to_path_buf()
        } else {
            std::env::current_dir()?.join(binary)
        }
    } else {
        which::which(binary).map_err(|_| {
            crate::Error::validation(format!("command not found: {}", binary.display()))
        })?
    };
    if !found.is_file() {
        bail!("command not found: {}", found.display());
    }
    if found.is_absolute() {
        return Ok(found);
    }
    Ok(std::env::current_dir()?.join(found))
}

/// Path to execute, and the symlink directory to keep on `PATH` when it differs.
///
/// A standalone Codex install is `~/.local/bin/codex` →
/// `~/.codex/packages/standalone/current/codex`, and `current` is another symlink.
/// Mounting only the final directory does not recreate `current`, so executing the
/// symlink fails inside the mount namespace. The canonical file is executed instead.
/// The symlink's directory stays on `PATH` so `#!/usr/bin/env node` can see a sibling
/// `node`.
fn canonical_command(argv0: &Path) -> Result<(PathBuf, Option<String>)> {
    let launcher = resolve_host_binary(argv0)?;
    let exec_binary = fs::canonicalize(&launcher).map_err(|_| {
        crate::Error::validation(format!("command not found: {}", launcher.display()))
    })?;
    if !exec_binary.is_file() {
        bail!("command not found: {}", exec_binary.display());
    }
    let exec_parent = exec_binary.parent();
    let launcher_dir = match launcher.parent() {
        Some(dir) if !dir.as_os_str().is_empty() && exec_parent != Some(dir) => {
            Some(path_to_string(dir)?)
        }
        _ => None,
    };
    Ok((exec_binary, launcher_dir))
}

fn path_to_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| crate::Error::validation("command path is not valid Unicode"))
}

#[cfg(target_os = "linux")]
fn which_first(binary: &Path) -> Option<PathBuf> {
    if binary.components().count() > 1 {
        return Some(binary.to_path_buf());
    }
    which::which(binary).ok()
}

#[cfg(target_os = "linux")]
struct LinuxFilter {
    table: String,
    cgroup: PathBuf,
}

#[cfg(target_os = "linux")]
impl LinuxFilter {
    fn cgroup(&self) -> &Path {
        &self.cgroup
    }
}

#[cfg(target_os = "linux")]
fn install_linux_filter(run_id: &str, proxy: &proxy::ResolvedProxy) -> Result<LinuxFilter> {
    let cgroup_name = format!("skilllite-{}", plan::sanitize_hostname(run_id));
    let parent = user_cgroup_dir()?;
    let cgroup = parent.join(&cgroup_name);
    fs::create_dir_all(&cgroup).map_err(|_| {
        crate::Error::validation("proxy enforcement unavailable: cannot create a private cgroup")
    })?;
    let table = format!("skilllite_{}", plan::sanitize_hostname(run_id));
    let script = plan::proxy_nft_script(&table, &cgroup, proxy)?;
    let mut child = Command::new("nft")
        .arg("-f")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| crate::Error::validation("proxy enforcement unavailable: nft is required"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let _ = fs::remove_dir(&cgroup);
        bail!("proxy enforcement unavailable: nft rejected the allowlist");
    }
    Ok(LinuxFilter { table, cgroup })
}

#[cfg(target_os = "linux")]
fn user_cgroup_dir() -> Result<PathBuf> {
    let text = fs::read_to_string("/proc/self/cgroup").map_err(|_| {
        crate::Error::validation("proxy enforcement unavailable: cannot read cgroup")
    })?;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            let full = PathBuf::from("/sys/fs/cgroup").join(path.trim_start_matches('/'));
            if full.is_dir() {
                return Ok(full);
            }
        }
    }
    bail!("proxy enforcement unavailable: cgroup v2 path was not found")
}

#[cfg(target_os = "linux")]
impl Drop for LinuxFilter {
    fn drop(&mut self) {
        let _ = Command::new("nft")
            .args(["delete", "table", "inet", &self.table])
            .output();
        let _ = fs::remove_dir(&self.cgroup);
    }
}

#[cfg(target_os = "linux")]
fn arm_linux_child(cmd: &mut Command, procs: PathBuf) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(move || {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let pid = libc::getpid();
            fs::write(&procs, format!("{pid}")).map_err(|err| {
                std::io::Error::other(format!("proxy enforcement unavailable: {err}"))
            })?;
            Ok(())
        });
    }
}

/// Default command for `runtime`.
///
/// Codex uses `--dangerously-bypass-approvals-and-sandbox` because this
/// process is already inside bwrap or Seatbelt. Codex's own sandbox does not
/// start there. Writes stay limited to the ephemeral home and the work
/// directory. Network stays limited to `https_proxy` when one is set.
#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
fn default_argv(runtime: AgentRuntime, prompt: &str) -> Vec<String> {
    match runtime {
        AgentRuntime::Claude => vec![
            "claude".to_string(),
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--permission-mode".to_string(),
            "dontAsk".to_string(),
        ],
        AgentRuntime::Codex => vec![
            "codex".to_string(),
            "exec".to_string(),
            "--json".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            prompt.to_string(),
        ],
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn persist_runtime_auth(runtime: AgentRuntime, home: &Path, account_dir: &Path) -> Result<()> {
    if runtime == AgentRuntime::Codex {
        persist_codex_auth(home, account_dir)?;
    }
    Ok(())
}

/// Copy a refreshed Codex `auth.json` back to the subscription directory.
///
/// Codex rotates the ChatGPT session in this file. The sandbox does not mount
/// the original directory, so the host copies this one regular file after the
/// process exits. Symlinks are refused.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn persist_codex_auth(home: &Path, account_dir: &Path) -> Result<()> {
    let src = home.join(".codex").join("auth.json");
    let meta = match fs::symlink_metadata(&src) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if !meta.file_type().is_file() {
        bail!("codex auth.json is not a regular file");
    }
    let dest_dir = account_dir.join(".codex");
    fs::create_dir_all(&dest_dir)?;
    let tmp = dest_dir.join(".auth.json.tmp");
    fs::copy(&src, &tmp)?;
    fs::rename(&tmp, dest_dir.join("auth.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn lock_tests() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[cfg(target_os = "linux")]
    fn fixture_root(name: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("agent-sandbox-fixtures");
        path.push(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(target_os = "linux")]
    struct RemoveDir(PathBuf);

    #[cfg(target_os = "linux")]
    impl Drop for RemoveDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(target_os = "linux")]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn missing_proxy_does_not_require_a_sandbox_binary() {
        let err = proxy::resolve_proxy("").unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[cfg(unix)]
    #[test]
    fn second_lock_on_the_same_directory_is_busy() {
        let _guard = lock_tests().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let _first = try_lock_directory(dir.path()).unwrap();
        let err = try_lock_directory(dir.path()).unwrap_err();
        assert!(err.to_string().contains("busy"));
    }

    #[cfg(unix)]
    #[test]
    fn copy_skips_symlinks_and_keeps_regular_files() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("src");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("nested").join("settings.json"), "{\"ok\":true}").unwrap();
        std::os::unix::fs::symlink(root.path().join("outside"), src.join("leak")).unwrap();
        let dst = root.path().join("dst");
        copy_tree(&src, &dst).unwrap();
        assert_eq!(
            fs::read_to_string(dst.join("nested").join("settings.json")).unwrap(),
            "{\"ok\":true}"
        );
        assert!(!dst.join("leak").exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_run_redacts_stdout_and_deletes_the_ephemeral_home() {
        let _guard = lock_tests().lock().unwrap();
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        fs::write(account.path().join(".claude").join("marker"), "keep").unwrap();
        let mut lines = Vec::new();
        let outcome = run(
            RunRequest {
                argv: vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf '%s\\n' \"$HOME\" 'token sk-ant-abc 中文'".into(),
                ],
                ..RunRequest::new(account.path(), work.path(), "修测试", "http://127.0.0.1:9")
            },
            |line| lines.push(line.to_string()),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("[redacted]") && line.contains("中文")),
            "lines={lines:?}"
        );
        let home = lines
            .iter()
            .find(|line| line.starts_with('/'))
            .expect("home path");
        assert!(
            !Path::new(home).exists(),
            "ephemeral home still exists: {home}"
        );
        assert_eq!(
            fs::read_to_string(account.path().join(".claude").join("marker")).unwrap(),
            "keep"
        );
        assert!(work.path().is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn missing_command_does_not_start() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let mut request = RunRequest::new(account.path(), work.path(), "prompt", "");
        request.argv = vec!["skilllite-missing-command-xyz".into()];
        let err = run(request, |_| {}).unwrap_err();
        assert!(err.to_string().contains("command not found"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_host_binary_keeps_the_symlink_path() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("echo-link");
        std::os::unix::fs::symlink("/bin/echo", &link).unwrap();
        let resolved = resolve_host_binary(&link).unwrap();
        assert_eq!(resolved, link);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_command_resolves_a_multihop_symlink_and_keeps_the_launcher_dir() {
        let root = tempfile::tempdir().unwrap();
        let release = root
            .path()
            .join("home/.codex/packages/standalone/releases/v1");
        let bin = release.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let real = bin.join("codex");
        fs::write(&real, b"bin").unwrap();
        std::os::unix::fs::symlink("bin/codex", release.join("codex")).unwrap();
        let standalone = root.path().join("home/.codex/packages/standalone");
        std::os::unix::fs::symlink(&release, standalone.join("current")).unwrap();
        let launcher = root.path().join("local/bin");
        fs::create_dir_all(&launcher).unwrap();
        let link = launcher.join("codex");
        std::os::unix::fs::symlink(standalone.join("current/codex"), &link).unwrap();

        let (exec, extra) = canonical_command(&link).unwrap();
        assert_eq!(exec, fs::canonicalize(&real).unwrap());
        assert_eq!(extra.as_deref(), launcher.to_str());
    }

    #[cfg(unix)]
    #[test]
    fn canonical_command_omits_the_launcher_dir_for_a_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        fs::write(&tool, b"x").unwrap();
        let (exec, extra) = canonical_command(&tool).unwrap();
        assert_eq!(exec, fs::canonicalize(&tool).unwrap());
        assert_eq!(extra, None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_run_executes_a_multihop_symlink() {
        let _guard = lock_tests().lock().unwrap();
        let root = fixture_root("multihop");
        let _cleanup = RemoveDir(root.clone());
        let release = root.join("home/.codex/packages/standalone/releases/v1");
        let bin = release.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::copy("/bin/echo", bin.join("codex")).unwrap();
        std::os::unix::fs::symlink("bin/codex", release.join("codex")).unwrap();
        let standalone = root.join("home/.codex/packages/standalone");
        std::os::unix::fs::symlink(&release, standalone.join("current")).unwrap();
        let launcher = root.join("local/bin");
        fs::create_dir_all(&launcher).unwrap();
        let link = launcher.join("codex");
        std::os::unix::fs::symlink(standalone.join("current/codex"), &link).unwrap();
        let account = root.join("account");
        let work = root.join("work");
        fs::create_dir_all(account.join(".claude")).unwrap();
        fs::create_dir_all(&work).unwrap();

        let mut lines = Vec::new();
        let outcome = run(
            RunRequest {
                argv: vec![link.to_string_lossy().into_owned(), "标记".into()],
                ..RunRequest::new(&account, &work, "prompt", "")
            },
            |line| lines.push(line.to_string()),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert!(lines.iter().any(|line| line == "标记"), "lines={lines:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_run_finds_node_beside_a_symlink_launcher() {
        let _guard = lock_tests().lock().unwrap();
        let root = fixture_root("node-launcher");
        let _cleanup = RemoveDir(root.clone());
        let module = root.join("nvm/lib/node_modules/@openai/codex/bin");
        let launcher = root.join("nvm/bin");
        fs::create_dir_all(&module).unwrap();
        fs::create_dir_all(&launcher).unwrap();
        let node = launcher.join("node");
        fs::write(&node, "#!/bin/sh\nprintf '%s\\n' 'node-ok'\n").unwrap();
        make_executable(&node);
        let script = module.join("codex.js");
        fs::write(&script, "#!/usr/bin/env node\n").unwrap();
        make_executable(&script);
        std::os::unix::fs::symlink(
            "../lib/node_modules/@openai/codex/bin/codex.js",
            launcher.join("codex"),
        )
        .unwrap();
        let account = root.join("account");
        let work = root.join("work");
        fs::create_dir_all(account.join(".claude")).unwrap();
        fs::create_dir_all(&work).unwrap();

        let mut lines = Vec::new();
        let outcome = run(
            RunRequest {
                argv: vec![launcher.join("codex").to_string_lossy().into_owned()],
                ..RunRequest::new(&account, &work, "prompt", "")
            },
            |line| lines.push(line.to_string()),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert!(
            lines.iter().any(|line| line == "node-ok"),
            "lines={lines:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn absolute_command_outside_the_default_path_runs() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let script = bin.path().join("agent-cmd");
        fs::write(&script, "#!/bin/sh\nprintf '%s\\n' ok\n").unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        let mut lines = Vec::new();
        let outcome = run(
            RunRequest {
                argv: vec![script.to_string_lossy().into_owned()],
                ..RunRequest::new(account.path(), work.path(), "prompt", "")
            },
            |line| lines.push(line.to_string()),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert!(lines.iter().any(|line| line == "ok"), "lines={lines:?}");
    }

    #[cfg(unix)]
    #[test]
    fn require_proxy_refuses_to_start_without_a_proxy() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let mut request = RunRequest::new(account.path(), work.path(), "prompt", "  ");
        request.require_proxy = true;
        request.argv = vec!["/bin/echo".into(), "should-not-run".into()];
        let err = run(request, |_| {}).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[cfg(unix)]
    #[test]
    fn a_bad_proxy_still_refuses_to_start_when_the_switch_is_off() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let mut request = RunRequest::new(
            account.path(),
            work.path(),
            "prompt",
            "socks5://127.0.0.1:9",
        );
        request.require_proxy = false;
        request.argv = vec!["/bin/echo".into(), "should-not-run".into()];
        let err = run(request, |_| {}).unwrap_err();
        assert!(err.to_string().contains("http://"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_proxy_starts_when_require_proxy_is_off() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let mut lines = Vec::new();
        let outcome = run(
            RunRequest {
                argv: vec!["/bin/echo".into(), "ok".into()],
                ..RunRequest::new(account.path(), work.path(), "prompt", "")
            },
            |line| lines.push(line.to_string()),
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert!(lines.iter().any(|line| line == "ok"), "lines={lines:?}");
    }

    #[test]
    fn runtime_name_accepts_claude_and_codex_only() {
        assert_eq!(
            AgentRuntime::from_name("claude").unwrap(),
            AgentRuntime::Claude
        );
        assert_eq!(
            AgentRuntime::from_name("codex").unwrap(),
            AgentRuntime::Codex
        );
        let err = AgentRuntime::from_name("gpt").unwrap_err();
        assert!(err.to_string().contains("claude or codex"));
    }

    #[test]
    fn default_commands_keep_each_runtime_prompt() {
        let claude = default_argv(AgentRuntime::Claude, "修测试");
        assert_eq!(claude.first().map(String::as_str), Some("claude"));
        assert!(claude.windows(2).any(|pair| pair == ["-p", "修测试"]));
        assert!(claude.iter().any(|arg| arg == "dontAsk"));
        let codex = default_argv(AgentRuntime::Codex, "修测试");
        assert_eq!(
            codex,
            vec![
                "codex",
                "exec",
                "--json",
                "--dangerously-bypass-approvals-and-sandbox",
                "修测试",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_auth_writeback_replaces_only_a_regular_file() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let account = root.path().join("account");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(account.join(".codex")).unwrap();
        fs::write(home.join(".codex").join("auth.json"), "refreshed 中文").unwrap();
        fs::write(account.join(".codex").join("config.toml"), "stay").unwrap();
        persist_codex_auth(&home, &account).unwrap();
        assert_eq!(
            fs::read_to_string(account.join(".codex").join("auth.json")).unwrap(),
            "refreshed 中文"
        );
        assert_eq!(
            fs::read_to_string(account.join(".codex").join("config.toml")).unwrap(),
            "stay"
        );
        fs::remove_file(account.join(".codex").join("auth.json")).unwrap();
        fs::remove_file(home.join(".codex").join("auth.json")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", home.join(".codex").join("auth.json")).unwrap();
        let err = persist_codex_auth(&home, &account).unwrap_err();
        assert!(err.to_string().contains("regular file"));
        assert!(!account.join(".codex").join("auth.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn missing_codex_directory_does_not_start() {
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        let mut request = RunRequest::new(account.path(), work.path(), "prompt", "");
        request.runtime = AgentRuntime::Codex;
        request.argv = vec!["/bin/echo".into(), "should-not-run".into()];
        let err = run(request, |_| {}).unwrap_err();
        assert!(err.to_string().contains(".codex"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn codex_run_writes_auth_json_back_and_leaves_other_files() {
        let _guard = lock_tests().lock().unwrap();
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".codex")).unwrap();
        fs::write(account.path().join(".codex").join("auth.json"), "old").unwrap();
        fs::write(account.path().join(".codex").join("config.toml"), "stay").unwrap();
        let outcome = run(
            RunRequest {
                runtime: AgentRuntime::Codex,
                argv: vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf '%s' 'refreshed 中文' > \"$HOME/.codex/auth.json\"; printf '%s' changed > \"$HOME/.codex/config.toml\"".into(),
                ],
                ..RunRequest::new(account.path(), work.path(), "prompt", "")
            },
            |_| {},
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert_eq!(
            fs::read_to_string(account.path().join(".codex").join("auth.json")).unwrap(),
            "refreshed 中文"
        );
        assert_eq!(
            fs::read_to_string(account.path().join(".codex").join("config.toml")).unwrap(),
            "stay"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn claude_run_does_not_write_the_login_directory_back() {
        let _guard = lock_tests().lock().unwrap();
        let account = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        fs::create_dir_all(account.path().join(".claude")).unwrap();
        fs::write(account.path().join(".claude").join("marker"), "keep").unwrap();
        let outcome = run(
            RunRequest {
                argv: vec![
                    "/bin/sh".into(),
                    "-c".into(),
                    "printf '%s' changed > \"$HOME/.claude/marker\"".into(),
                ],
                ..RunRequest::new(account.path(), work.path(), "prompt", "")
            },
            |_| {},
        )
        .unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(outcome.exit_code, 0, "stderr={}", outcome.stderr_excerpt);
        assert_eq!(
            fs::read_to_string(account.path().join(".claude").join("marker")).unwrap(),
            "keep"
        );
    }
}
