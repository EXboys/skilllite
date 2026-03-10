//! 通用工具：is_likely_binary, matches_glob

use std::path::Path;

use crate::read_write;

/// 检测文件是否可能为二进制（前 512 字节含 null）
pub fn is_likely_binary(path: &Path) -> bool {
    if let Ok(buf) = read_write::read_bytes_limit(path, 512) {
        return buf.contains(&0);
    }
    true
}

/// 简单 glob 匹配：支持 *.ext 或精确文件名
pub fn matches_glob(name: &str, pattern: &str) -> bool {
    if let Some(ext) = pattern.strip_prefix("*.") {
        name.ends_with(&format!(".{}", ext))
    } else {
        name == pattern
    }
}
