//! Prompts/memory/skills snapshot create, restore, and pruning.

use std::path::Path;

use crate::error::bail;
use crate::Result;
use skilllite_core::config::env_keys::evolution as evo_keys;

// ─── Snapshots ────────────────────────────────────────────────────────────────

pub(crate) fn versions_dir(chat_root: &Path) -> std::path::PathBuf {
    chat_root.join("prompts").join("_versions")
}

/// How many evolution txn snapshot directories to keep under `prompts/_versions/`.
/// `0` = keep all (no pruning). Default `10`. Invalid env falls back to default.
fn evolution_snapshot_keep_count() -> usize {
    match std::env::var(evo_keys::SKILLLITE_EVOLUTION_SNAPSHOT_KEEP)
        .ok()
        .as_deref()
    {
        Some(s) if !s.is_empty() => s.parse::<usize>().unwrap_or(10),
        _ => 10,
    }
}

pub fn create_snapshot(chat_root: &Path, txn_id: &str, files: &[&str]) -> Result<Vec<String>> {
    let snap_dir = versions_dir(chat_root).join(txn_id);
    std::fs::create_dir_all(&snap_dir)?;
    let prompts = chat_root.join("prompts");
    let mut backed_up = Vec::new();
    for name in files {
        let src = prompts.join(name);
        if src.exists() {
            let dst = snap_dir.join(name);
            std::fs::copy(&src, &dst)?;
            backed_up.push(name.to_string());
        }
    }
    prune_snapshots(chat_root, evolution_snapshot_keep_count());
    Ok(backed_up)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub(crate) fn create_extended_snapshot(
    chat_root: &Path,
    skills_root: Option<&Path>,
    txn_id: &str,
    include_prompts: bool,
    include_memory: bool,
    include_skills: bool,
) -> Result<Vec<String>> {
    let mut backed_up = Vec::new();
    if include_prompts {
        backed_up.extend(create_snapshot(
            chat_root,
            txn_id,
            &[
                "rules.json",
                "examples.json",
                "planning.md",
                "execution.md",
                "system.md",
            ],
        )?);
    } else {
        let snap_dir = versions_dir(chat_root).join(txn_id);
        std::fs::create_dir_all(&snap_dir)?;
    }

    let snap_dir = versions_dir(chat_root).join(txn_id);
    if include_memory {
        let memory_src = chat_root
            .join("memory")
            .join("evolution")
            .join("knowledge.md");
        if memory_src.exists() {
            let memory_dst = snap_dir.join("memory").join("knowledge.md");
            if let Some(parent) = memory_dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(memory_src, memory_dst)?;
            backed_up.push("memory/evolution/knowledge.md".to_string());
        }
    }

    if include_skills {
        if let Some(sr) = skills_root {
            let evolved_src = sr.join("_evolved");
            if evolved_src.exists() {
                let evolved_dst = snap_dir.join("skills").join("_evolved");
                copy_dir_recursive(&evolved_src, &evolved_dst)?;
                backed_up.push("skills/_evolved".to_string());
            }
        }
    }

    prune_snapshots(chat_root, evolution_snapshot_keep_count());
    Ok(backed_up)
}

pub fn restore_snapshot(chat_root: &Path, txn_id: &str) -> Result<()> {
    let snap_dir = versions_dir(chat_root).join(txn_id);
    if !snap_dir.exists() {
        bail!("Snapshot not found: {}", txn_id);
    }
    let prompts = chat_root.join("prompts");
    for entry in std::fs::read_dir(&snap_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let dst = prompts.join(entry.file_name());
        std::fs::copy(entry.path(), &dst)?;
    }
    tracing::info!("Restored snapshot {}", txn_id);
    Ok(())
}

pub(crate) fn restore_extended_snapshot(
    chat_root: &Path,
    skills_root: Option<&Path>,
    txn_id: &str,
) -> Result<()> {
    restore_snapshot(chat_root, txn_id)?;
    let snap_dir = versions_dir(chat_root).join(txn_id);

    let memory_src = snap_dir.join("memory").join("knowledge.md");
    if memory_src.exists() {
        let memory_dst = chat_root
            .join("memory")
            .join("evolution")
            .join("knowledge.md");
        if let Some(parent) = memory_dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(memory_src, memory_dst)?;
    }

    let skills_src = snap_dir.join("skills").join("_evolved");
    if skills_src.exists() {
        if let Some(sr) = skills_root {
            let skills_dst = sr.join("_evolved");
            if skills_dst.exists() {
                std::fs::remove_dir_all(&skills_dst)?;
            }
            copy_dir_recursive(&skills_src, &skills_dst)?;
        }
    }
    Ok(())
}

fn prune_snapshots(chat_root: &Path, keep: usize) {
    if keep == 0 {
        return;
    }
    let vdir = versions_dir(chat_root);
    if !vdir.exists() {
        return;
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&vdir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    if dirs.len() <= keep {
        return;
    }
    dirs.sort_by_key(|e| e.file_name());
    let to_remove = dirs.len() - keep;
    for entry in dirs.into_iter().take(to_remove) {
        let _ = std::fs::remove_dir_all(entry.path());
    }
}
