//! Build isolated runtime environments (Python venv / Node) and resolve RuntimePaths.
//!
//! Accepts [`skilllite_core::EnvSpec`] only; callers build it from `SkillMetadata` via
//! `EnvSpec::from_metadata(skill_dir, &metadata)` so that this crate does not depend on skill parsing.
//! P0: Prefer system Python/Node; if missing or version too low, provision via runtime_deps with
//! transparent progress reporting.

use anyhow::{Context, Result};
use skilllite_core::config;
use skilllite_core::EnvSpec;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::env::runtime_deps::{self, RuntimeConfirmDownloadFn, RuntimeProgressFn};
use crate::runner::RuntimePaths;

/// Return the cache directory for skill environments.
/// If `override_dir` is provided, use it; otherwise read from env / config.
pub fn get_cache_dir(override_dir: Option<&str>) -> Option<PathBuf> {
    let base = override_dir
        .map(PathBuf::from)
        .or_else(|| {
            config::load_dotenv();
            config::CacheConfig::cache_dir().map(PathBuf::from)
        })
        .or_else(|| dirs::cache_dir().map(|d| d.join("skilllite")))?;
    Some(base.join("envs"))
}

/// Ensure an isolated environment exists for the skill (venv or node_modules).
/// Returns the environment directory path (empty PathBuf if no env needed, e.g. bash-only).
/// P0: If system Python/Node is missing or too old, progress callback reports reason and
/// provisioning progress (pass None to skip). Desktop can pass e.g.
/// `Some(Box::new(|msg| { /* show in UI */ }))` for transparent UX.
/// Pass `confirm_download` to ask user before downloading a runtime; if it returns false, provisioning is aborted.
pub fn ensure_environment(
    skill_dir: &Path,
    spec: &EnvSpec,
    cache_dir: Option<&str>,
    progress: RuntimeProgressFn,
    confirm_download: RuntimeConfirmDownloadFn,
) -> Result<PathBuf> {
    let lang = &spec.language;

    let base = get_cache_dir(cache_dir).unwrap_or_else(|| {
        PathBuf::from(".")
            .join(".cache")
            .join("skilllite")
            .join("envs")
    });
    std::fs::create_dir_all(&base).context("Create cache dir")?;

    let key = cache_key(skill_dir, spec, lang)?;
    let env_path = base.join(key);

    if lang == "python" {
        ensure_python_env(
            skill_dir,
            spec,
            &env_path,
            cache_dir,
            progress,
            confirm_download,
        )?;
    } else if lang == "node" {
        ensure_node_env(
            skill_dir,
            spec,
            &env_path,
            cache_dir,
            progress,
            confirm_download,
        )?;
    } else {
        return Ok(PathBuf::new());
    }

    Ok(env_path)
}

/// Build RuntimePaths from an environment directory (or empty for system interpreters).
pub fn build_runtime_paths(env_dir: &Path) -> RuntimePaths {
    // Read bundled node path from marker file written by ensure_node_env
    let resolve_node_bin = |dir: &Path| -> PathBuf {
        let marker = dir.join(".skilllite_node_bin");
        if let Ok(contents) = std::fs::read_to_string(&marker) {
            let p = PathBuf::from(contents.trim());
            if p.exists() {
                return p;
            }
        }
        PathBuf::from("node")
    };

    if env_dir.as_os_str().is_empty() || !env_dir.exists() {
        return RuntimePaths {
            python: default_system_python_command(),
            node: PathBuf::from("node"),
            node_modules: None,
            env_dir: PathBuf::new(),
        };
    }

    let node = resolve_node_bin(env_dir);

    let (python, node_modules) = if env_dir.join("bin").join("python").exists() {
        (env_dir.join("bin").join("python"), None::<PathBuf>)
    } else if env_dir.join("Scripts").join("python.exe").exists() {
        (env_dir.join("Scripts").join("python.exe"), None)
    } else if env_dir.join("node_modules").exists() {
        (
            default_system_python_command(),
            Some(env_dir.join("node_modules")),
        )
    } else {
        (default_system_python_command(), None)
    };

    RuntimePaths {
        python,
        node,
        node_modules,
        env_dir: env_dir.to_path_buf(),
    }
}

fn cache_key(skill_dir: &Path, spec: &EnvSpec, lang: &str) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(
        skill_dir
            .canonicalize()
            .unwrap_or_else(|_| skill_dir.to_path_buf())
            .to_string_lossy()
            .as_bytes(),
    );
    hasher.update(lang.as_bytes());
    if let Some(ref pkgs) = spec.resolved_packages {
        for p in pkgs {
            hasher.update(p.as_bytes());
        }
    }
    let lock_path = skill_dir.join(".skilllite.lock");
    if lock_path.exists() {
        let _ = std::fs::read_to_string(&lock_path).map(|c| hasher.update(c.as_bytes()));
    }
    let req = skill_dir.join("requirements.txt");
    if req.exists() {
        let _ = std::fs::read_to_string(&req).map(|c| hasher.update(c.as_bytes()));
    }
    let pkg = skill_dir.join("package.json");
    if pkg.exists() {
        let _ = std::fs::read_to_string(&pkg).map(|c| hasher.update(c.as_bytes()));
    }
    Ok(hex::encode(hasher.finalize()))
}

fn ensure_python_env(
    skill_dir: &Path,
    spec: &EnvSpec,
    env_path: &Path,
    cache_dir: Option<&str>,
    progress: RuntimeProgressFn,
    confirm_download: RuntimeConfirmDownloadFn,
) -> Result<()> {
    let packages = collect_python_packages(skill_dir, spec)?;
    let python_path = python_path_in_env(env_path);
    let env_exists = python_path.exists();

    if !env_exists {
        std::fs::create_dir_all(env_path).context("Create venv dir")?;

        let python = resolve_python(cache_dir, progress, confirm_download)?;
        let mut cmd = Command::new(&python.program);
        cmd.args(&python.args).arg("-m").arg("venv").arg(env_path);
        cmd.current_dir(skill_dir);
        let out = cmd.output().context("Create venv")?;
        if !out.status.success() {
            anyhow::bail!("venv failed: {}", String::from_utf8_lossy(&out.stderr));
        }

        if !packages.is_empty() {
            let pip = pip_path_in_env(env_path);
            let mut cmd = if pip.file_name().map(|n| n == "python").unwrap_or(false) {
                let mut c = Command::new(&pip);
                c.arg("-m").arg("pip").arg("install");
                c
            } else {
                let mut c = Command::new(&pip);
                c.arg("install");
                c
            };
            cmd.args(&packages).current_dir(skill_dir);
            let out = cmd.output().context("pip install")?;
            if !out.status.success() {
                anyhow::bail!(
                    "pip install failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    }

    if requests_playwright_browsers(&packages) {
        install_playwright_browsers_for_python(skill_dir, env_path)?;
    }

    Ok(())
}

fn ensure_node_env(
    skill_dir: &Path,
    spec: &EnvSpec,
    env_path: &Path,
    cache_dir: Option<&str>,
    progress: RuntimeProgressFn,
    confirm_download: RuntimeConfirmDownloadFn,
) -> Result<()> {
    let packages = collect_node_packages(skill_dir, spec)?;
    let env_exists = env_path.join("node_modules").exists();

    // Always ensure node/npm are available (even if no deps to install),
    // so build_runtime_paths can find the bundled node binary later.
    let (node_bin, _npm_path) = resolve_node(cache_dir, progress, confirm_download)?;

    // Write a marker so build_runtime_paths knows where node lives
    std::fs::create_dir_all(env_path).context("Create node env dir")?;
    let node_bin_marker = env_path.join(".skilllite_node_bin");
    std::fs::write(&node_bin_marker, node_bin.to_string_lossy().as_bytes())
        .context("Write node bin marker")?;

    if !env_exists {
        let package_json = skill_dir.join("package.json");
        let has_deps = if package_json.exists() {
            std::fs::copy(&package_json, env_path.join("package.json"))
                .context("Copy package.json")?;
            true
        } else if let Some(ref pkgs) = spec.resolved_packages {
            let deps: std::collections::HashMap<String, String> =
                pkgs.iter().map(|p| (p.clone(), "*".to_string())).collect();
            let pkg = serde_json::json!({
                "name": "skill-env",
                "version": "1.0.0",
                "private": true,
                "dependencies": deps
            });
            std::fs::write(
                env_path.join("package.json"),
                serde_json::to_string_pretty(&pkg).context("Serialize package.json")?,
            )
            .context("Write package.json")?;
            true
        } else {
            false
        };

        if has_deps {
            let lock = skill_dir.join("package-lock.json");
            if lock.exists() {
                let _ = std::fs::copy(&lock, env_path.join("package-lock.json"));
            }
            let out = Command::new(&_npm_path)
                .args(["install", "--omit=dev"])
                .current_dir(env_path)
                .output()
                .context("npm install")?;
            if !out.status.success() {
                anyhow::bail!(
                    "npm install failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    }

    if requests_playwright_browsers(&packages) {
        install_playwright_browsers_for_node(env_path)?;
    }

    Ok(())
}

fn collect_python_packages(skill_dir: &Path, spec: &EnvSpec) -> Result<Vec<String>> {
    if let Some(ref pkgs) = spec.resolved_packages {
        return Ok(pkgs.clone());
    }

    let req = skill_dir.join("requirements.txt");
    if !req.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&req).context("Read requirements.txt")?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

fn collect_node_packages(skill_dir: &Path, spec: &EnvSpec) -> Result<Vec<String>> {
    if let Some(ref pkgs) = spec.resolved_packages {
        return Ok(pkgs.clone());
    }

    let package_json = skill_dir.join("package.json");
    if !package_json.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&package_json).context("Read package.json")?;
    let package_json: serde_json::Value =
        serde_json::from_str(&content).context("Parse package.json")?;
    let mut packages = Vec::new();

    for section in ["dependencies", "devDependencies"] {
        if let Some(map) = package_json.get(section).and_then(|v| v.as_object()) {
            packages.extend(map.keys().cloned());
        }
    }

    Ok(packages)
}

fn python_path_in_env(env_path: &Path) -> PathBuf {
    let unix = env_path.join("bin").join("python");
    if unix.exists() {
        unix
    } else {
        env_path.join("Scripts").join("python.exe")
    }
}

fn pip_path_in_env(env_path: &Path) -> PathBuf {
    let pip_bin = env_path.join("bin").join("pip");
    let pip_scripts = env_path.join("Scripts").join("pip.exe");
    if pip_bin.exists() {
        pip_bin
    } else if pip_scripts.exists() {
        pip_scripts
    } else {
        python_path_in_env(env_path)
    }
}

fn requests_playwright_browsers(packages: &[String]) -> bool {
    packages.iter().any(|pkg| {
        let base = pkg.split(['=', '<', '>', '!', '~']).next().unwrap_or(pkg);
        let normalized = base
            .split_once('[')
            .map(|(name, _)| name)
            .unwrap_or(base)
            .trim()
            .to_lowercase()
            .replace('_', "-");
        normalized == "playwright" || normalized == "@playwright/test"
    })
}

fn install_playwright_browsers_for_python(skill_dir: &Path, env_path: &Path) -> Result<()> {
    let python = python_path_in_env(env_path);
    if !python.exists() {
        anyhow::bail!("playwright browser install skipped: python env missing");
    }

    let out = Command::new(&python)
        .args(["-m", "playwright", "install", "chromium"])
        .current_dir(skill_dir)
        .output()
        .context("playwright install chromium")?;
    if !out.status.success() {
        anyhow::bail!(
            "playwright install chromium failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(())
}

fn install_playwright_browsers_for_node(env_path: &Path) -> Result<()> {
    let unix_cli = env_path
        .join("node_modules")
        .join(".bin")
        .join("playwright");
    let windows_cli = env_path
        .join("node_modules")
        .join(".bin")
        .join("playwright.cmd");

    let mut cmd = if unix_cli.exists() {
        let mut cmd = Command::new(unix_cli);
        cmd.args(["install", "chromium"]);
        cmd
    } else if windows_cli.exists() {
        let mut cmd = Command::new(windows_cli);
        cmd.args(["install", "chromium"]);
        cmd
    } else {
        let mut cmd = Command::new("npm");
        cmd.args(["exec", "--", "playwright", "install", "chromium"]);
        cmd
    };

    let out = cmd
        .current_dir(env_path)
        .output()
        .context("playwright install chromium")?;
    if !out.status.success() {
        anyhow::bail!(
            "playwright install chromium failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(())
}

fn default_system_python_command() -> PathBuf {
    default_system_python_command_for_platform(cfg!(windows))
}

fn default_system_python_command_for_platform(is_windows: bool) -> PathBuf {
    if is_windows {
        PathBuf::from("python")
    } else {
        PathBuf::from("python3")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PythonCommand {
    program: PathBuf,
    args: Vec<&'static str>,
}

fn which_python() -> Result<PythonCommand> {
    for candidate in python_command_candidates() {
        let mut cmd = Command::new(&candidate.program);
        cmd.args(&candidate.args).arg("--version");
        let out = cmd.output();
        if let Ok(ref o) = out {
            if o.status.success() {
                return Ok(candidate);
            }
        }
    }
    anyhow::bail!("no usable Python launcher found in PATH")
}

/// Prefer system Python (if version >= MIN_PYTHON_VERSION); otherwise provision to ~/.skilllite/runtime/.
fn resolve_python(
    cache_dir: Option<&str>,
    progress: RuntimeProgressFn,
    confirm_download: RuntimeConfirmDownloadFn,
) -> Result<PythonCommand> {
    if let Ok(cmd) = which_python() {
        let out = Command::new(&cmd.program)
            .args(&cmd.args)
            .arg("--version")
            .output();
        if let Ok(ref o) = out {
            if let Some(ver) = std::str::from_utf8(&o.stdout)
                .or_else(|_| std::str::from_utf8(&o.stderr))
                .ok()
                .and_then(runtime_deps::parse_python_version)
            {
                if runtime_deps::python_version_meets_minimum(ver.0, ver.1) {
                    return Ok(cmd);
                }
            }
        }
    }
    let runtime_dir = runtime_deps::get_runtime_dir(cache_dir)
        .context("Cannot determine runtime dir for Python")?;
    let python_bin_path = runtime_dir.join("python-3.12").join("bin").join("python");
    #[cfg(windows)]
    let python_bin_path = runtime_dir.join("python-3.12").join("python.exe");
    if !python_bin_path.exists() {
        if let Some(ref confirm) = confirm_download {
            let req = runtime_deps::RuntimeDownloadRequest::python();
            if !confirm(&req) {
                anyhow::bail!(
                    "Runtime download declined. To allow automatic download, set SKILLLITE_AUTO_APPROVE_RUNTIME=1."
                );
            }
        }
    }
    let python_bin = runtime_deps::ensure_python_runtime(&runtime_dir, progress)?;
    Ok(PythonCommand {
        program: python_bin,
        args: Vec::new(),
    })
}

/// Prefer system Node/npm (if version >= MIN_NODE_MAJOR); otherwise provision to ~/.skilllite/runtime/.
fn resolve_node(
    cache_dir: Option<&str>,
    progress: RuntimeProgressFn,
    confirm_download: RuntimeConfirmDownloadFn,
) -> Result<(PathBuf, PathBuf)> {
    if let (Some(node), Some(npm)) = (runtime_deps::which_node(), runtime_deps::which_npm()) {
        return Ok((node, npm));
    }
    let runtime_dir = runtime_deps::get_runtime_dir(cache_dir)
        .context("Cannot determine runtime dir for Node")?;
    let node_bin_path = runtime_dir.join("node-20").join("bin").join("node");
    #[cfg(windows)]
    let node_bin_path = runtime_dir.join("node-20").join("node.exe");
    if !node_bin_path.exists() {
        if let Some(ref confirm) = confirm_download {
            let req = runtime_deps::RuntimeDownloadRequest::node();
            if !confirm(&req) {
                anyhow::bail!(
                    "Runtime download declined. To allow automatic download, set SKILLLITE_AUTO_APPROVE_RUNTIME=1."
                );
            }
        }
    }
    runtime_deps::ensure_node_runtime(&runtime_dir, progress)
}

fn python_command_candidates() -> Vec<PythonCommand> {
    python_command_candidates_for_platform(cfg!(windows))
}

fn python_command_candidates_for_platform(is_windows: bool) -> Vec<PythonCommand> {
    if is_windows {
        vec![
            PythonCommand {
                program: PathBuf::from("python"),
                args: Vec::new(),
            },
            PythonCommand {
                program: PathBuf::from("py"),
                args: vec!["-3"],
            },
            PythonCommand {
                program: PathBuf::from("python3"),
                args: Vec::new(),
            },
            PythonCommand {
                program: PathBuf::from("py"),
                args: Vec::new(),
            },
        ]
    } else {
        vec![
            PythonCommand {
                program: PathBuf::from("python3"),
                args: Vec::new(),
            },
            PythonCommand {
                program: PathBuf::from("python"),
                args: Vec::new(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_system_python_command_matches_platform() {
        if cfg!(windows) {
            assert_eq!(default_system_python_command(), PathBuf::from("python"));
        } else {
            assert_eq!(default_system_python_command(), PathBuf::from("python3"));
        }
    }

    #[test]
    fn test_python_command_candidates_include_platform_fallbacks() {
        let candidates = python_command_candidates();
        assert!(!candidates.is_empty());

        if cfg!(windows) {
            assert_eq!(candidates[0].program, PathBuf::from("python"));
            assert!(candidates
                .iter()
                .any(|c| c.program == PathBuf::from("py") && c.args == vec!["-3"]));
        } else {
            assert_eq!(candidates[0].program, PathBuf::from("python3"));
            assert!(candidates
                .iter()
                .any(|c| c.program == PathBuf::from("python")));
        }
    }

    #[test]
    fn test_windows_python_launcher_candidates_are_ranked_for_launcher_compat() {
        let candidates = python_command_candidates_for_platform(true);
        let actual: Vec<(PathBuf, Vec<&'static str>)> = candidates
            .into_iter()
            .map(|c| (c.program, c.args))
            .collect();

        assert_eq!(
            actual,
            vec![
                (PathBuf::from("python"), vec![]),
                (PathBuf::from("py"), vec!["-3"]),
                (PathBuf::from("python3"), vec![]),
                (PathBuf::from("py"), vec![]),
            ]
        );
    }

    #[test]
    fn test_default_system_python_command_uses_platform_specific_fallback() {
        assert_eq!(
            default_system_python_command_for_platform(true),
            PathBuf::from("python")
        );
        assert_eq!(
            default_system_python_command_for_platform(false),
            PathBuf::from("python3")
        );
    }

    #[test]
    fn test_build_runtime_paths_prefers_windows_venv_python_exe() {
        let temp_dir = TempDir::new().expect("temp dir");
        let scripts_dir = temp_dir.path().join("Scripts");
        std::fs::create_dir_all(&scripts_dir).expect("create Scripts");
        std::fs::write(scripts_dir.join("python.exe"), b"").expect("create python.exe");

        let runtime = build_runtime_paths(temp_dir.path());

        assert_eq!(runtime.python, scripts_dir.join("python.exe"));
        assert_eq!(runtime.env_dir, temp_dir.path());
    }

    #[test]
    fn test_build_runtime_paths_prefers_unix_venv_python() {
        let temp_dir = TempDir::new().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir_all(&bin_dir).expect("create bin");
        std::fs::write(bin_dir.join("python"), b"").expect("create python");

        let runtime = build_runtime_paths(temp_dir.path());

        assert_eq!(runtime.python, bin_dir.join("python"));
        assert_eq!(runtime.env_dir, temp_dir.path());
    }

    #[test]
    fn test_collect_python_packages_from_requirements() {
        let temp_dir = TempDir::new().expect("temp dir");
        std::fs::write(
            temp_dir.path().join("requirements.txt"),
            "pyodps==0.12.5\n# comment\nplaywright\n",
        )
        .expect("write requirements");
        let spec = EnvSpec {
            language: "python".to_string(),
            name: Some("test".to_string()),
            compatibility: Some("Requires Python".to_string()),
            resolved_packages: None,
        };

        let packages =
            collect_python_packages(temp_dir.path(), &spec).expect("collect python packages");
        assert_eq!(
            packages,
            vec!["pyodps==0.12.5".to_string(), "playwright".to_string()]
        );
    }

    #[test]
    fn test_collect_node_packages_from_package_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{
  "dependencies": { "playwright": "^1.50.0" },
  "devDependencies": { "@anthropic-ai/sdk": "^0.39.0" }
}"#,
        )
        .expect("write package.json");
        let spec = EnvSpec {
            language: "node".to_string(),
            name: Some("test".to_string()),
            compatibility: Some("Requires Node.js".to_string()),
            resolved_packages: None,
        };

        let packages =
            collect_node_packages(temp_dir.path(), &spec).expect("collect node packages");
        assert!(packages.contains(&"playwright".to_string()));
        assert!(packages.contains(&"@anthropic-ai/sdk".to_string()));
    }

    #[test]
    fn test_requests_playwright_browsers_handles_versions_and_extras() {
        assert!(requests_playwright_browsers(&[
            "playwright==1.51.0".to_string(),
            "requests".to_string(),
        ]));
        assert!(requests_playwright_browsers(&[
            "@playwright/test".to_string()
        ]));
        assert!(!requests_playwright_browsers(&[
            "playwright-core".to_string()
        ]));
    }
}
