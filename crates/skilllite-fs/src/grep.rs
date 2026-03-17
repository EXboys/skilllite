//! 递归 grep：按 regex 搜索目录

use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::read_write;
use crate::util;

/// 默认跳过的目录
pub const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    "venv",
    ".venv",
    ".tox",
];

/// 单条匹配：(相对路径, 行号, 行内容)
pub type GrepMatch = (String, usize, String);

/// 递归 grep 目录，返回匹配行
///
/// - `base`: 用于生成相对路径的基准，若为 None 则使用完整路径
/// - `include`: 可选 glob，如 "*.rs" 仅匹配扩展名
/// - `skip_dirs`: 跳过的目录名，默认用 SKIP_DIRS
/// - `max_matches`: 最大匹配数
pub fn grep_directory(
    path: &Path,
    re: &Regex,
    base: Option<&Path>,
    include: Option<&str>,
    skip_dirs: &[&str],
    max_matches: usize,
) -> Result<(Vec<GrepMatch>, usize)> {
    let mut results = Vec::new();
    let mut files_matched = 0usize;
    grep_recursive(
        path,
        base,
        re,
        include,
        skip_dirs,
        max_matches,
        &mut results,
        &mut files_matched,
    )?;
    Ok((results, files_matched))
}

fn grep_recursive(
    dir: &Path,
    base: Option<&Path>,
    re: &Regex,
    include: Option<&str>,
    skip_dirs: &[&str],
    max_matches: usize,
    results: &mut Vec<GrepMatch>,
    files_matched: &mut usize,
) -> Result<()> {
    if !dir.is_dir() {
        return grep_single_file(dir, base, re, results, files_matched, max_matches);
    }
    let entries = crate::dir::read_dir(dir)?;
    for (path, is_dir) in entries {
        if results.len() >= max_matches {
            return Ok(());
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if is_dir {
            if skip_dirs.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            grep_recursive(
                &path,
                base,
                re,
                include,
                skip_dirs,
                max_matches,
                results,
                files_matched,
            )?;
        } else {
            if let Some(glob) = include {
                if !util::matches_glob(&name, glob) {
                    continue;
                }
            }
            if util::is_likely_binary(&path) {
                continue;
            }
            grep_single_file(&path, base, re, results, files_matched, max_matches)?;
        }
    }
    Ok(())
}

fn grep_single_file(
    path: &Path,
    base: Option<&Path>,
    re: &Regex,
    results: &mut Vec<GrepMatch>,
    files_matched: &mut usize,
    max_matches: usize,
) -> Result<()> {
    let content = match read_write::read_file(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let rel_path = base
        .and_then(|b| path.strip_prefix(b).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let mut file_has_match = false;
    for (line_num, line) in content.lines().enumerate() {
        if results.len() >= max_matches {
            break;
        }
        if re.is_match(line) {
            if !file_has_match {
                *files_matched += 1;
                file_has_match = true;
            }
            results.push((rel_path.clone(), line_num + 1, line.to_string()));
        }
    }
    Ok(())
}
