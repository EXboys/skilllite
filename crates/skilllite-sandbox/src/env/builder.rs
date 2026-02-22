//! Build isolated runtime environments (Python venv / Node) and resolve RuntimePaths.

use anyhow::{Context, Result};
use skilllite_core::config;
use skilllite_core::skill::metadata;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        .or_else(|| {
            dirs::cache_dir().map(|d| d.join("skilllite"))
        })?;
    Some(base.join("envs"))
}

/// Ensure an isolated environment exists for the skill (venv or node_modules).
/// Returns the environment directory path (empty PathBuf if no env needed, e.g. bash-only).
pub fn ensure_environment(
    skill_dir: &Path,
    meta: &metadata::SkillMetadata,
    cache_dir: Option<&str>,
) -> Result<PathBuf> {
    let lang = metadata::detect_language(skill_dir, meta);
    // Bash-tool skills (e.g. agent-browser) may have Node deps (package.json) — install them
    let lang = if lang == "bash" {
        let has_pkg = skill_dir.join("package.json").exists();
        let compat_has_agent_browser = meta
            .compatibility
            .as_ref()
            .map_or(false, |c| c.to_lowercase().contains("agent-browser"));
        let resolved_has_agent_browser = meta
            .resolved_packages
            .as_ref()
            .map_or(false, |p| p.iter().any(|s| s.contains("agent-browser")));
        if has_pkg || compat_has_agent_browser || resolved_has_agent_browser {
            "node".to_string()
        } else {
            return Ok(PathBuf::new());
        }
    } else {
        lang
    };

    let base = get_cache_dir(cache_dir)
        .unwrap_or_else(|| PathBuf::from(".").join(".cache").join("skilllite").join("envs"));
    std::fs::create_dir_all(&base).context("Create cache dir")?;

    let key = cache_key(skill_dir, meta, &lang)?;
    let env_path = base.join(key);

    if lang == "python" {
        ensure_python_env(skill_dir, meta, &env_path)?;
    } else if lang == "node" {
        ensure_node_env(skill_dir, meta, &env_path)?;
    } else {
        return Ok(PathBuf::new());
    }

    Ok(env_path)
}

/// Build RuntimePaths from an environment directory (or empty for system interpreters).
pub fn build_runtime_paths(env_dir: &Path) -> RuntimePaths {
    if env_dir.as_os_str().is_empty() || !env_dir.exists() {
        return RuntimePaths {
            python: PathBuf::from("python3"),
            node: PathBuf::from("node"),
            node_modules: None,
            env_dir: PathBuf::new(),
        };
    }

    let (python, node, node_modules) = if env_dir.join("bin").join("python").exists() {
        (
            env_dir.join("bin").join("python"),
            PathBuf::from("node"),
            None::<PathBuf>,
        )
    } else if env_dir.join("Scripts").join("python.exe").exists() {
        (
            env_dir.join("Scripts").join("python.exe"),
            PathBuf::from("node"),
            None,
        )
    } else if env_dir.join("node_modules").exists() {
        (
            PathBuf::from("python3"),
            PathBuf::from("node"),
            Some(env_dir.join("node_modules")),
        )
    } else {
        (
            PathBuf::from("python3"),
            PathBuf::from("node"),
            None,
        )
    };

    RuntimePaths {
        python,
        node,
        node_modules,
        env_dir: env_dir.to_path_buf(),
    }
}

fn cache_key(skill_dir: &Path, meta: &metadata::SkillMetadata, lang: &str) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(skill_dir.canonicalize().unwrap_or_else(|_| skill_dir.to_path_buf()).to_string_lossy().as_bytes());
    hasher.update(lang.as_bytes());
    if let Some(ref pkgs) = meta.resolved_packages {
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

fn ensure_python_env(skill_dir: &Path, meta: &metadata::SkillMetadata, env_path: &Path) -> Result<()> {
    if env_path.join("bin").join("python").exists() || env_path.join("Scripts").join("python.exe").exists() {
        return Ok(());
    }

    std::fs::create_dir_all(env_path).context("Create venv dir")?;

    let python3 = which_python()?;
    let mut cmd = Command::new(&python3);
    cmd.arg("-m").arg("venv").arg(env_path);
    cmd.current_dir(skill_dir);
    let out = cmd.output().context("Create venv")?;
    if !out.status.success() {
        anyhow::bail!(
            "venv failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let pip_bin = env_path.join("bin").join("pip");
    let pip_scripts = env_path.join("Scripts").join("pip.exe");
    let pip = if pip_bin.exists() {
        pip_bin
    } else if pip_scripts.exists() {
        pip_scripts
    } else {
        env_path.join("bin").join("python") // fallback: python -m pip
    };

    let packages: Vec<String> = if let Some(ref pkgs) = meta.resolved_packages {
        pkgs.clone()
    } else {
        let req = skill_dir.join("requirements.txt");
        if req.exists() {
            let content = std::fs::read_to_string(&req).context("Read requirements.txt")?;
            content
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect()
        } else {
            return Ok(());
        }
    };

    if packages.is_empty() {
        return Ok(());
    }

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

    Ok(())
}

fn ensure_node_env(skill_dir: &Path, meta: &metadata::SkillMetadata, env_path: &Path) -> Result<()> {
    if env_path.join("node_modules").exists() {
        return Ok(());
    }

    std::fs::create_dir_all(env_path).context("Create node env dir")?;

    let package_json = skill_dir.join("package.json");
    if package_json.exists() {
        std::fs::copy(&package_json, env_path.join("package.json")).context("Copy package.json")?;
    } else if let Some(ref pkgs) = meta.resolved_packages {
        // Bash-tool skills without package.json may list deps in .skilllite.lock (from skilllite init)
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
    } else {
        return Ok(());
    }
    let lock = skill_dir.join("package-lock.json");
    if lock.exists() {
        let _ = std::fs::copy(&lock, env_path.join("package-lock.json"));
    }

    let out = Command::new("npm")
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

    Ok(())
}

fn which_python() -> Result<PathBuf> {
    for name in ["python3", "python"] {
        let out = Command::new(name).arg("--version").output();
        if out.is_ok() && out.as_ref().unwrap().status.success() {
            return Ok(PathBuf::from(name));
        }
    }
    anyhow::bail!("python3 or python not found in PATH")
}
