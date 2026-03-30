//! 最近文件、计划、在文件管理器中打开目录、按相对路径读取聊天目录下文件。

use base64::Engine;
use serde::Serialize;

use super::paths::{
    skilllite_chat_root, validate_chat_subdir_relative, validate_transcript_log_filename,
};

/// Task step from plan JSON.
#[derive(Debug, Clone, Serialize)]
pub struct PlanStep {
    pub id: u32,
    pub description: String,
    pub completed: bool,
}

/// Plan from ~/.skilllite/chat/plans/.
#[derive(Debug, Clone, Serialize)]
pub struct RecentPlan {
    pub task: String,
    pub steps: Vec<PlanStep>,
}

/// Recent data: memory files, output files, latest plan.
#[derive(Debug, Clone, Serialize)]
pub struct RecentData {
    pub memory_files: Vec<String>,
    pub output_files: Vec<String>,
    pub log_files: Vec<String>,
    pub plan: Option<RecentPlan>,
}

/// Open a directory in the system file manager.
pub fn open_directory(module: &str) -> Result<(), String> {
    let chat_root = skilllite_chat_root();
    let path = match module {
        "output" => chat_root.join("output"),
        "memory" => chat_root.join("memory"),
        "plan" => chat_root.join("plans"),
        "log" => chat_root.join("transcripts"),
        "evolution" => chat_root,
        _ => return Err(format!("Unknown module: {}", module)),
    };
    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn reveal_in_file_manager(path_str: &str) -> Result<(), String> {
    let raw = std::path::PathBuf::from(path_str.trim());
    if !raw.is_absolute() {
        return Err("需要绝对路径".to_string());
    }
    let path = raw.clone();
    if !path.exists() {
        let rt = skilllite_sandbox::get_runtime_dir(None)
            .ok_or_else(|| "无法解析 SkillLite 运行时目录".to_string())?;
        if raw == rt {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        } else {
            return Err("路径不存在".to_string());
        }
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    reveal_path_in_os(&path)
}

fn reveal_path_in_os(path: &std::path::Path) -> Result<(), String> {
    if path.is_dir() {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if path.is_file() {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        {
            use std::ffi::OsString;
            let mut arg = OsString::from("/select,");
            arg.push(path.as_os_str());
            std::process::Command::new("explorer")
                .arg(arg)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let parent = path
                .parent()
                .ok_or_else(|| "无法解析文件所在目录".to_string())?;
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Err("不是有效的文件或目录".to_string())
}

type FileWithMtime = (String, std::time::SystemTime);

fn collect_md_files(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<FileWithMtime>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_md_files(&p, base, out);
        } else if p.extension().map_or(false, |e| e == "md") {
            if let Ok(rel) = p.strip_prefix(base) {
                let mtime = p
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                out.push((rel.to_string_lossy().to_string(), mtime));
            }
        }
    }
}

fn collect_output_files_inner(
    dir: &std::path::Path,
    base: &std::path::Path,
    out: &mut Vec<FileWithMtime>,
) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    const EXTS: &[&str] = &[
        "md", "html", "htm", "txt", "json", "csv", "png", "jpg", "jpeg", "gif", "webp", "svg",
    ];
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_output_files_inner(&p, base, out);
        } else if let Some(ext) = p.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if EXTS.contains(&ext_lower.as_str()) {
                if let Ok(rel) = p.strip_prefix(base) {
                    let mtime = p
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    out.push((rel.to_string_lossy().to_string(), mtime));
                }
            }
        }
    }
}

fn sort_newest_first(mut items: Vec<FileWithMtime>) -> Vec<String> {
    items.sort_by(|a, b| b.1.cmp(&a.1));
    items.into_iter().map(|(path, _)| path).collect()
}

fn load_memory_files(chat_root: &std::path::Path) -> Vec<String> {
    let memory_dir = chat_root.join("memory");
    let mut out = Vec::new();
    if memory_dir.exists() {
        collect_md_files(&memory_dir, &memory_dir, &mut out);
    }
    sort_newest_first(out)
}

fn load_log_files(chat_root: &std::path::Path) -> Vec<String> {
    let transcripts_dir = chat_root.join("transcripts");
    if !transcripts_dir.is_dir() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(&transcripts_dir) else {
        return vec![];
    };
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(3 * 24 * 60 * 60);
    let mut out: Vec<FileWithMtime> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().map_or(true, |e| e != "jsonl") {
            continue;
        }
        let mtime = p
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if mtime < cutoff {
            continue;
        }
        if let Some(name) = p.file_name() {
            out.push((name.to_string_lossy().to_string(), mtime));
        }
    }
    sort_newest_first(out)
}

pub fn read_log_file(filename: &str) -> Result<String, String> {
    validate_transcript_log_filename(filename)?;
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("transcripts").join(filename);
    if !full_path.starts_with(&chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

fn load_output_files(chat_root: &std::path::Path) -> Vec<String> {
    let output_dir = chat_root.join("output");
    let mut out = Vec::new();
    if output_dir.exists() {
        collect_output_files_inner(&output_dir, &output_dir, &mut out);
    }
    sort_newest_first(out)
}

fn load_plan_data(chat_root: &std::path::Path) -> Option<RecentPlan> {
    let plans_dir = chat_root.join("plans");
    if !plans_dir.exists() {
        return None;
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let session_key = "default";

    fn parse_plan_from_file(path: &std::path::Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        match path.extension().and_then(|e| e.to_str()) {
            Some("jsonl") => content
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .and_then(|l| serde_json::from_str(l).ok()),
            _ => serde_json::from_str(&content).ok(),
        }
    }

    let plan: Option<serde_json::Value> = {
        let jsonl_path = plans_dir.join(format!("{}-{}.jsonl", session_key, today));
        let json_path = plans_dir.join(format!("{}-{}.json", session_key, today));
        if jsonl_path.exists() {
            parse_plan_from_file(&jsonl_path)
        } else if json_path.exists() {
            parse_plan_from_file(&json_path)
        } else {
            let mut candidates: Vec<_> = std::fs::read_dir(&plans_dir)
                .ok()?
                .flatten()
                .filter(|e| {
                    e.path()
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map_or(false, |n| n.starts_with(session_key))
                })
                .collect();
            candidates.sort_by_key(|e| {
                std::fs::metadata(e.path())
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });
            candidates
                .last()
                .and_then(|e| parse_plan_from_file(&e.path()))
        }
    };

    let plan = plan?;
    let task = plan
        .get("task")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let steps_arr = plan.get("steps").and_then(|s| s.as_array())?;
    let steps: Vec<PlanStep> = steps_arr
        .iter()
        .map(|s| {
            let id = s.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let desc = s.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let status = s
                .get("status")
                .and_then(|st| st.as_str())
                .unwrap_or("pending");
            PlanStep {
                id: if id > 0 { id } else { 1 },
                description: desc.to_string(),
                completed: status == "completed" || status == "done",
            }
        })
        .collect();
    Some(RecentPlan { task, steps })
}

pub fn load_recent() -> RecentData {
    let chat_root = skilllite_chat_root();
    if !chat_root.exists() {
        return RecentData {
            memory_files: vec![],
            output_files: vec![],
            log_files: vec![],
            plan: None,
        };
    }

    let root = chat_root.clone();
    let mem_handle = std::thread::spawn(move || load_memory_files(&root));

    let root = chat_root.clone();
    let out_handle = std::thread::spawn(move || load_output_files(&root));

    let root = chat_root.clone();
    let log_handle = std::thread::spawn(move || load_log_files(&root));

    let plan_handle = std::thread::spawn(move || load_plan_data(&chat_root));

    let memory_files = mem_handle.join().unwrap_or_default();
    let output_files = out_handle.join().unwrap_or_default();
    let log_files = log_handle.join().unwrap_or_default();
    let plan = plan_handle.join().unwrap_or(None);

    RecentData {
        memory_files,
        output_files,
        log_files,
        plan,
    }
}

pub fn read_output_file(relative_path: &str) -> Result<String, String> {
    validate_chat_subdir_relative(relative_path)?;
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("output").join(relative_path);
    if !full_path.starts_with(&chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

pub fn read_output_file_base64(relative_path: &str) -> Result<String, String> {
    validate_chat_subdir_relative(relative_path)?;
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("output").join(relative_path);
    if !full_path.starts_with(&chat_root) {
        return Err("Path escape".to_string());
    }
    let bytes = std::fs::read(&full_path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub fn read_memory_file(relative_path: &str) -> Result<String, String> {
    validate_chat_subdir_relative(relative_path)?;
    let chat_root = skilllite_chat_root();
    let full_path = chat_root.join("memory").join(relative_path);
    if !full_path.starts_with(&chat_root) {
        return Err("Path escape".to_string());
    }
    std::fs::read_to_string(&full_path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryEntry {
    pub path: String,
    pub title: String,
    pub summary: String,
    pub updated_at: String,
}

pub fn load_memory_summaries() -> Vec<MemoryEntry> {
    let chat_root = skilllite_chat_root();
    let memory_dir = chat_root.join("memory");
    if !memory_dir.exists() {
        return vec![];
    }

    let mut files: Vec<FileWithMtime> = Vec::new();
    collect_md_files(&memory_dir, &memory_dir, &mut files);
    files.sort_by(|a, b| b.1.cmp(&a.1));

    files
        .into_iter()
        .take(30)
        .map(|(rel_path, mtime)| {
            let full_path = memory_dir.join(&rel_path);
            let content = std::fs::read_to_string(&full_path).unwrap_or_default();
            let title = content
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim_start_matches('#').trim().to_string())
                .unwrap_or_else(|| rel_path.clone());
            let summary: String = content
                .lines()
                .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            let summary = if summary.chars().count() > 120 {
                format!("{}…", summary.chars().take(120).collect::<String>())
            } else {
                summary
            };

            let updated_secs = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            MemoryEntry {
                path: rel_path,
                title,
                summary,
                updated_at: format!("{}", updated_secs),
            }
        })
        .collect()
}
