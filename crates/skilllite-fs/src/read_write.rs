//! 读写原语：read_file, write_file, append_file, atomic_write, search_replace

use std::path::Path;

use anyhow::{Context, Result};

use crate::dir;

/// 读取文件为 UTF-8 字符串
pub fn read_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// 读取文件为原始字节
pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// 读取文件前 `limit` 字节（用于二进制检测等）
pub fn read_bytes_limit(path: &Path, limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut buf = vec![0u8; limit];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// 写入文件（覆盖），UTF-8
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        dir::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write file: {}", path.display()))
}

/// 追加写入文件
pub fn append_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        dir::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open file for append: {}", path.display()))?;
    f.write_all(content.as_bytes())
        .with_context(|| format!("Failed to append to file: {}", path.display()))?;
    Ok(())
}

/// 原子写入：先写 .tmp 再 rename
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        dir::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)
        .with_context(|| format!("Failed to write temp file: {}", tmp.display()))?;
    dir::rename(&tmp, path)?;
    Ok(())
}

/// 在文件内做精确 search_replace，返回替换次数
pub fn search_replace(path: &Path, old_string: &str, new_string: &str, replace_all: bool) -> Result<usize> {
    let content = read_file(path)?;
    let (new_content, count) = crate::search_replace::apply_search_replace(
        &content,
        old_string,
        new_string,
        replace_all,
    )?;
    if count > 0 {
        write_file(path, &new_content)?;
    }
    Ok(count)
}
