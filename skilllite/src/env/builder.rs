use crate::config::env_keys;
use crate::skill::deps::{detect_dependencies, get_cache_key, DependencyType};
use crate::skill::metadata::SkillMetadata;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Subdirectory name for cached skill environments (under cache root).
const ENV_CACHE_SUBDIR: &str = "skilllite";
const ENV_CACHE_ENVS: &str = "envs";

/// Marker file indicating environment setup is complete.
const ENV_MARKER_FILE: &str = ".skilllite_complete";
/// Legacy marker (backward compatibility with agentskill).
const ENV_MARKER_LEGACY: &str = ".agentskill_complete";

/// Get the default cache directory for environments.
/// - `custom_cache_dir`: CLI override (full path)
/// - `SKILLLITE_CACHE_DIR` / `AGENTSKILL_CACHE_DIR`: env override (full path)
/// - Default: `{system_cache}/skilllite/envs`
pub fn get_cache_dir(custom_cache_dir: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = custom_cache_dir {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var(env_keys::SKILLLITE_CACHE_DIR) {
        return Ok(PathBuf::from(dir));
    }
    if let Ok(dir) = std::env::var(env_keys::AGENTSKILL_CACHE_DIR) {
        return Ok(PathBuf::from(dir));
    }

    let base = get_system_cache_dir()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not determine cache directory. Please set {} or XDG_CACHE_HOME environment variable.",
                env_keys::SKILLLITE_CACHE_DIR
            )
        })?;
    Ok(base.join(ENV_CACHE_SUBDIR).join(ENV_CACHE_ENVS))
}

/// Get the system cache directory without external dependencies.
/// Follows XDG Base Directory Specification on Unix-like systems.
fn get_system_cache_dir() -> Option<PathBuf> {

    #[cfg(target_os = "macos")]
    {
        // macOS: ~/Library/Caches
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library").join("Caches"))
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Follow XDG spec, default to ~/.cache
        if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
            Some(PathBuf::from(xdg_cache))
        } else {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache"))
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: %LOCALAPPDATA%
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Ensure the environment is set up for the skill
/// Dependencies are now parsed from the compatibility field in SKILL.md
pub fn ensure_environment(
    skill_dir: &Path,
    metadata: &SkillMetadata,
    custom_cache_dir: Option<&str>,
) -> Result<PathBuf> {
    let dep_info = detect_dependencies(skill_dir, metadata)?;
    let cache_dir = get_cache_dir(custom_cache_dir)?;

    // Create cache directory if it doesn't exist
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;

    let cache_key = get_cache_key(&dep_info);
    let env_path = cache_dir.join(&cache_key);

    match dep_info.dep_type {
        DependencyType::Python => {
            // Install Python packages from compatibility field
            ensure_python_env(&env_path, &dep_info.packages)?;
        }
        DependencyType::Node => {
            // Install Node.js packages from compatibility field
            ensure_node_env(&env_path, skill_dir, &dep_info.packages)?;
        }
        DependencyType::None => {
            // No dependencies, use system environment
            return Ok(PathBuf::new());
        }
    }

    Ok(env_path)
}

/// Ensure Python virtual environment exists and has dependencies installed
/// Packages are parsed from the compatibility field
fn ensure_python_env(env_path: &Path, packages: &[String]) -> Result<()> {
    // Check if environment already exists and is complete (accept both new and legacy marker)
    let marker = env_path.join(ENV_MARKER_FILE);
    let marker_legacy = env_path.join(ENV_MARKER_LEGACY);
    if env_path.exists() && (marker.exists() || marker_legacy.exists()) {
        return Ok(());
    }

    // Remove incomplete environment if exists
    if env_path.exists() {
        fs::remove_dir_all(env_path)?;
    }

    // Create virtual environment
    create_python_venv(env_path)?;

    // Install packages from compatibility field
    if !packages.is_empty() {
        install_python_packages(env_path, packages)?;
    }

    // Create marker file to indicate completion
    fs::write(&marker, "")?;

    Ok(())
}

/// Install Python packages from compatibility field
fn install_python_packages(env_path: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let pip_path = get_python_pip_path(env_path);

    let mut args = vec![
        "install".to_string(),
        "--quiet".to_string(),
        "--disable-pip-version-check".to_string(),
    ];
    args.extend(packages.iter().cloned());

    let output = Command::new(&pip_path)
        .args(&args)
        .output()
        .with_context(|| "Failed to execute pip install")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to install packages from compatibility: {}", stderr);
    }

    Ok(())
}

/// Create a Python virtual environment
fn create_python_venv(env_path: &Path) -> Result<()> {
    let output = Command::new("python3")
        .args(["-m", "venv", env_path.to_str().expect("env path must be valid UTF-8")])
        .output()
        .with_context(|| "Failed to execute python3 -m venv")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create virtual environment: {}", stderr);
    }

    Ok(())
}

/// Get the path to pip in the virtual environment
fn get_python_pip_path(env_path: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        env_path.join("Scripts").join("pip")
    } else {
        env_path.join("bin").join("pip")
    }
}

/// Get the path to python in the virtual environment
pub fn get_python_executable(env_path: &Path) -> PathBuf {
    if env_path.as_os_str().is_empty() {
        // No virtual environment, use system python
        PathBuf::from("python3")
    } else if cfg!(target_os = "windows") {
        env_path.join("Scripts").join("python")
    } else {
        env_path.join("bin").join("python")
    }
}

/// Ensure Node.js environment exists and has dependencies installed
/// Packages are parsed from the compatibility field
fn ensure_node_env(env_path: &Path, _skill_dir: &Path, packages: &[String]) -> Result<()> {
    let marker = env_path.join(ENV_MARKER_FILE);
    let marker_legacy = env_path.join(ENV_MARKER_LEGACY);
    if env_path.exists() && (marker.exists() || marker_legacy.exists()) {
        return Ok(());
    }

    // Remove incomplete environment if exists
    if env_path.exists() {
        fs::remove_dir_all(env_path)?;
    }

    // Create environment directory
    fs::create_dir_all(env_path)?;

    // Install packages from compatibility field using npm
    if !packages.is_empty() {
        let mut args = vec!["install".to_string(), "--silent".to_string()];
        args.extend(packages.iter().cloned());

        let output = Command::new("npm")
            .args(&args)
            .current_dir(env_path)
            .output()
            .with_context(|| "Failed to execute npm install")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to install node packages from compatibility: {}", stderr);
        }
    }

    // Create marker file
    fs::write(&marker, "")?;

    Ok(())
}

/// Get the path to node executable
pub fn get_node_executable() -> PathBuf {
    PathBuf::from("node")
}

/// Get the node_modules path for a skill
pub fn get_node_modules_path(env_path: &Path) -> PathBuf {
    env_path.join("node_modules")
}
