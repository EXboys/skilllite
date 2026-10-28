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

/// Inputs for one subscription sandbox.
///
/// `argv`, when empty, becomes
/// `claude -p <prompt> --output-format stream-json --verbose --permission-mode dontAsk`.
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
    let claude_home = request.account_dir.join(".claude");
    if !claude_home.is_dir() {
        bail!("account_dir has no .claude directory");
    }
    let resolved = optional_proxy(&request)?;
    let _lock = try_lock_directory(&request.account_dir)?;
    let home = tempfile::TempDir::new()?;
    copy_tree(&claude_home, &home.path().join(".claude"))?;
    let home_path = fs::canonicalize(home.path())?;
    let work_path = fs::canonicalize(&request.work_dir)?;
    let run_id = if request.run_id.is_empty() {
        generated_run_id()
    } else {
        request.run_id.clone()
    };
    let argv = if request.argv.is_empty() {
        vec![
            "claude".to_string(),
            "-p".to_string(),
            request.prompt.clone(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--permission-mode".to_string(),
            "dontAsk".to_string(),
        ]
    } else {
        request.argv.clone()
    };
    let binary = Path::new(&argv[0]);
    let resolved_binary = which_first(binary);
    let layout = plan::detect_layout(resolved_binary.as_deref());

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
                    return Err(err.into());
                }
            }
        }
    }
    let status = child.wait()?;
    group.disarm();
    stop.store(true, Ordering::Relaxed);
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
        let _ = (run_id, layout);
        let profile = plan::seatbelt_profile(home, work, proxy)?;
        let profile_path = home.join("sandbox.sb");
        fs::write(&profile_path, profile)?;
        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f").arg(&profile_path);
        cmd.arg(&argv[0]);
        cmd.args(&argv[1..]);
        cmd.current_dir(work);
        apply_env(&mut cmd, home, proxy);
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn apply_env(cmd: &mut Command, home: &Path, proxy: Option<&proxy::ResolvedProxy>) {
    cmd.env_clear();
    cmd.env("HOME", home);
    cmd.env("TMPDIR", home.join("tmp"));
    let _ = fs::create_dir_all(home.join("tmp"));
    cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin");
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
    let level = plan::cgroup_v2_level(&cgroup).ok_or_else(|| {
        crate::Error::validation("proxy enforcement unavailable: cgroup is outside the v2 root")
    })?;
    let script = plan::nft_script(&table, &cgroup_name, level, proxy);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn lock_tests() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
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
