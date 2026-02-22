//! Environment management commands: clean cached virtual environments.
//!
//! Cached environments live in `~/.cache/skilllite/envs/` (or `$SKILLLITE_CACHE_DIR`).
//! Each subdirectory is a hash-keyed environment created by `ensure_environment()`.

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Get the cache directory for skill environments.
/// Uses `env::builder::get_cache_dir` for consistency.
fn get_cache_dir() -> PathBuf {
    skilllite_sandbox::env::builder::get_cache_dir(None).unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".cache"))
            .join("skilllite")
            .join("envs")
    })
}

/// `skilllite env clean`
pub fn cmd_clean(dry_run: bool, force: bool) -> Result<()> {
    let cache_dir = get_cache_dir();

    if !cache_dir.exists() {
        eprintln!("No cached environments found at {}", cache_dir.display());
        return Ok(());
    }

    let mut entries: Vec<(PathBuf, u64)> = Vec::new();
    let mut total_size: u64 = 0;

    if let Ok(dir_entries) = fs::read_dir(&cache_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let size = dir_size(&path);
                total_size += size;
                entries.push((path, size));
            }
        }
    }

    entries.sort_by_key(|e| e.0.file_name().unwrap_or_default().to_os_string());

    if entries.is_empty() {
        eprintln!("No cached environments found at {}", cache_dir.display());
        return Ok(());
    }

    eprintln!(
        "🗂  Cached environments ({}) in {}:",
        entries.len(),
        cache_dir.display()
    );
    eprintln!();
    for (path, size) in &entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        eprintln!("  • {} ({})", name, format_size(*size));
    }
    eprintln!();
    eprintln!("Total: {} ({} environments)", format_size(total_size), entries.len());

    if dry_run {
        eprintln!();
        eprintln!("(Dry run — no files removed. Remove --dry-run to delete.)");
        return Ok(());
    }

    // Confirm removal
    if !force {
        eprint!("\nRemove all cached environments? [y/N] ");
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    // Remove all cached environments
    let mut removed = 0;
    let mut errors = 0;
    for (path, _) in &entries {
        match fs::remove_dir_all(path) {
            Ok(()) => {
                removed += 1;
            }
            Err(e) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                eprintln!("  ✗ Failed to remove {}: {}", name, e);
                errors += 1;
            }
        }
    }

    eprintln!();
    if errors == 0 {
        eprintln!(
            "✓ Removed {} cached environment(s), freed {}",
            removed,
            format_size(total_size)
        );
    } else {
        eprintln!(
            "⚠ Removed {}/{} environments ({} errors)",
            removed,
            entries.len(),
            errors
        );
    }

    Ok(())
}

/// Compute total size of a directory recursively.
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Format byte size to human-readable string.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
