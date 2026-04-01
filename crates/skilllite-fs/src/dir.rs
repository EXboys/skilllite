//! 目录与路径操作

use std::path::Path;
use std::time::SystemTime;

use anyhow::Context;

use crate::{Error, Result};

/// 路径类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathKind {
    NotFound,
    File(u64),
    Dir,
}

/// 读取目录条目，返回 (完整路径, 是否目录)，已排序
pub fn read_dir(path: &Path) -> Result<Vec<(std::path::PathBuf, bool)>> {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .with_context(|| format!("Failed to read dir: {}", path.display()))?
        .filter_map(|e| e.ok())
        .map(|e| {
            let p = e.path();
            (p.clone(), p.is_dir())
        })
        .collect();
    entries.sort_by_key(|(p, _)| p.file_name().unwrap_or_default().to_owned());
    Ok(entries)
}

/// 确保目录存在
pub fn create_dir_all(path: &Path) -> Result<()> {
    Ok(std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create dir: {}", path.display()))?)
}

/// 复制文件
pub fn copy(from: &Path, to: &Path) -> Result<u64> {
    Ok(std::fs::copy(from, to)
        .with_context(|| format!("Failed to copy {} -> {}", from.display(), to.display()))?)
}

/// 重命名/移动
pub fn rename(from: &Path, to: &Path) -> Result<()> {
    Ok(std::fs::rename(from, to)
        .with_context(|| format!("Failed to rename {} -> {}", from.display(), to.display()))?)
}

/// 删除文件
pub fn remove_file(path: &Path) -> Result<()> {
    Ok(std::fs::remove_file(path)
        .with_context(|| format!("Failed to remove file: {}", path.display()))?)
}

/// 获取修改时间
pub fn modified_time(path: &Path) -> Result<SystemTime> {
    Ok(std::fs::metadata(path)
        .and_then(|m| m.modified())
        .with_context(|| format!("Failed to get modified time: {}", path.display()))?)
}

/// 检查路径是否存在及类型
pub fn file_exists(path: &Path) -> Result<PathKind> {
    match std::fs::metadata(path) {
        Ok(meta) => {
            if meta.is_dir() {
                Ok(PathKind::Dir)
            } else {
                Ok(PathKind::File(meta.len()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PathKind::NotFound),
        Err(e) => Err(e.into()),
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes / 1024)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else {
        format!("{} GB", bytes / (1024 * 1024 * 1024))
    }
}

/// 列出目录内容，返回排序后的条目
pub fn list_directory(path: &Path, recursive: bool) -> Result<Vec<String>> {
    if !path.exists() {
        return Err(Error::validation(format!(
            "Directory not found: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(Error::validation(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }
    let mut entries = Vec::new();
    list_dir_impl(path, path, recursive, &mut entries, 0)?;
    Ok(entries)
}

fn list_dir_impl(
    base: &Path,
    current: &Path,
    recursive: bool,
    entries: &mut Vec<String>,
    depth: usize,
) -> Result<()> {
    let skip_dirs = [
        "node_modules",
        "__pycache__",
        ".git",
        "venv",
        ".venv",
        ".tox",
        "target",
    ];

    let mut items: Vec<_> = std::fs::read_dir(current)
        .with_context(|| format!("Failed to read dir: {}", current.display()))?
        .filter_map(|e| e.ok())
        .collect();
    items.sort_by_key(|e| e.file_name());

    for entry in items {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.path().is_dir();

        let rel = entry
            .path()
            .strip_prefix(base)
            .unwrap_or(entry.path().as_path())
            .to_string_lossy()
            .to_string();

        if name.starts_with('.') && depth == 0 && name != "." {
            let prefix = if is_dir { "📁 " } else { "   " };
            entries.push(format!("{}{}", prefix, name));
            continue;
        }

        if is_dir {
            entries.push(format!("📁 {}/", rel));
            if recursive && !skip_dirs.contains(&name.as_str()) {
                list_dir_impl(base, &entry.path(), true, entries, depth + 1)?;
            }
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            entries.push(format!("   {} ({})", rel, format_size(size)));
        }
    }
    Ok(())
}
