//! 备份与清理

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::dir;

/// 备份文件到 backup_dir，返回备份路径
pub fn backup_file(file_path: &Path, backup_dir: &Path) -> Result<PathBuf> {
    dir::create_dir_all(backup_dir)?;
    let filename = file_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?
        .to_string_lossy()
        .to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_name = format!("{}_{}", ts, filename);
    let backup_path = backup_dir.join(&backup_name);
    dir::copy(file_path, &backup_path)?;
    Ok(backup_path)
}

/// 清理目录中最老的条目，保留 keep 个
pub fn prune_oldest_files(dir_path: &Path, keep: usize) {
    if let Ok(entries) = dir::read_dir(dir_path) {
        let mut files: Vec<PathBuf> = entries
            .into_iter()
            .filter(|(_, is_dir)| !is_dir)
            .map(|(p, _)| p)
            .collect();
        if files.len() <= keep {
            return;
        }
        files.sort_by_key(|p| dir::modified_time(p).unwrap_or(std::time::SystemTime::UNIX_EPOCH));
        for path in files.iter().take(files.len() - keep) {
            let _ = dir::remove_file(path);
        }
    }
}
