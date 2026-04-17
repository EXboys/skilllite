//! `chat_root/prompts` 与 `_versions/<txn>/` 下的 **prompt 工件**路径与读写边界。
//!
//! 桌面集成里存在两类「行级/白名单」契约：
//! - **agent-rpc**：见 [`crate::skilllite_bridge::protocol`] 的 `parse_stream_event_line` 等单测；
//! - **本模块**：允许的相对文件名、txn 目录名规则、字节上限。
//!
//! 与架构文档 **§2.8**（网络代理等跨进程契约）同一原则：**可测试的不变量用单测钉住**，避免 UI 与磁盘布局漂移。
//! 此处用 `EVOLUTION_PROMPT_DIFF_FILENAMES` 单源驱动校验与 `load_evolution_diffs` 迭代（见
//! [`evolution_prompt_diff_contract_tests`]）。

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Read live `prompts/<filename>` (not a txn snapshot). Used by the assistant UI version picker.
pub const PROMPT_VERSION_CURRENT: &str = "__current__";

const MAX_PROMPT_VERSION_BYTES: u64 = 2 * 1024 * 1024;

/// Basenames under `prompts/` participating in evolution diff / version UI.
/// **Single source** for allowlist checks and [`load_evolution_diffs`] iteration.
pub const EVOLUTION_PROMPT_DIFF_FILENAMES: &[&str] = &[
    "planning.md",
    "execution.md",
    "system.md",
    "examples.md",
    "rules.json",
    "examples.json",
];

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionFileDiffDto {
    pub filename: String,
    pub evolved: bool,
    pub content: String,
    pub original_content: Option<String>,
}

/// One evolution txn directory under `prompts/_versions/<txn_id>/` that contains a prompt file.
#[derive(Debug, Clone, Serialize)]
pub struct EvolutionSnapshotTxnDto {
    pub txn_id: String,
    /// Best-effort mtime of the snapshot file (seconds since UNIX epoch).
    pub modified_unix: i64,
}

fn evolution_prompt_filename_allowed(name: &str) -> bool {
    EVOLUTION_PROMPT_DIFF_FILENAMES.contains(&name)
}

fn safe_evolution_txn_dir_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 {
        return false;
    }
    if name == PROMPT_VERSION_CURRENT {
        return false;
    }
    if name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn read_utf8_file_capped(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Err("路径是目录".to_string());
    }
    let len = meta.len();
    if len > MAX_PROMPT_VERSION_BYTES {
        return Err(format!(
            "文件超过 {} 字节上限，无法在应用内对比",
            MAX_PROMPT_VERSION_BYTES
        ));
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// List txn snapshot dirs that contain `filename`, newest first (by snapshot file mtime).
pub fn list_prompt_snapshot_txns_at(
    chat_root: &Path,
    filename: &str,
) -> Result<Vec<EvolutionSnapshotTxnDto>, String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let versions_dir = chat_root.join("prompts").join("_versions");
    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(&versions_dir).map_err(|e| e.to_string())?;
    let mut out: Vec<EvolutionSnapshotTxnDto> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if !file_type.is_dir() {
            continue;
        }
        let txn_id = entry.file_name().to_string_lossy().into_owned();
        if !safe_evolution_txn_dir_name(&txn_id) {
            continue;
        }
        let snap_path = entry.path().join(filename);
        if !snap_path.is_file() {
            continue;
        }
        let modified_unix = std::fs::metadata(&snap_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        out.push(EvolutionSnapshotTxnDto {
            txn_id,
            modified_unix,
        });
    }
    out.sort_by_key(|t| std::cmp::Reverse(t.modified_unix));
    Ok(out)
}

/// Read one prompt `filename` from either live prompts or `_versions/<txn_id>/`.
pub fn read_prompt_snapshot_version_at(
    chat_root: &Path,
    filename: &str,
    version_ref: &str,
) -> Result<String, String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let trimmed = version_ref.trim();
    if trimmed == PROMPT_VERSION_CURRENT {
        let path = chat_root.join("prompts").join(filename);
        if !path.is_file() {
            return Ok(String::new());
        }
        return read_utf8_file_capped(&path);
    }
    if !safe_evolution_txn_dir_name(trimmed) {
        return Err("无效的版本标识".to_string());
    }
    let path = chat_root
        .join("prompts")
        .join("_versions")
        .join(trimmed)
        .join(filename);
    if !path.is_file() {
        return Err("该快照中无此文件".to_string());
    }
    read_utf8_file_capped(&path)
}

pub fn list_prompt_snapshot_txns(filename: &str) -> Result<Vec<EvolutionSnapshotTxnDto>, String> {
    list_prompt_snapshot_txns_at(&skilllite_core::paths::chat_root(), filename)
}

fn list_prompt_snapshots_batch_at(
    chat_root: &Path,
    filenames: &[String],
) -> Result<HashMap<String, Vec<EvolutionSnapshotTxnDto>>, String> {
    let mut out = HashMap::new();
    for name in filenames {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !evolution_prompt_filename_allowed(trimmed) {
            return Err(format!("不支持的 prompt 文件名: {}", trimmed));
        }
        let list = list_prompt_snapshot_txns_at(chat_root, trimmed)?;
        out.insert(trimmed.to_string(), list);
    }
    Ok(out)
}

/// 一次请求列出多个 prompt 文件的快照 txn（避免前端 N 路并行 invoke 与 Strict Mode 竞态）。
pub fn list_prompt_snapshots_batch(
    filenames: &[String],
) -> Result<HashMap<String, Vec<EvolutionSnapshotTxnDto>>, String> {
    list_prompt_snapshots_batch_at(&skilllite_core::paths::chat_root(), filenames)
}

pub fn read_prompt_version_content(filename: &str, version_ref: &str) -> Result<String, String> {
    read_prompt_snapshot_version_at(&skilllite_core::paths::chat_root(), filename, version_ref)
}

/// Write UTF-8 to `chat_root/prompts/<filename>`（仅允许与快照对比相同白名单）。
pub fn write_chat_prompt_text_file(filename: &str, content: &str) -> Result<(), String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let len = content.len() as u64;
    if len > MAX_PROMPT_VERSION_BYTES {
        return Err(format!("内容超过 {} 字节上限", MAX_PROMPT_VERSION_BYTES));
    }
    let path = skilllite_core::paths::chat_root()
        .join("prompts")
        .join(filename);
    skilllite_fs::write_file(&path, content).map_err(|e| e.to_string())
}

fn evolved_prompt_files_from_changelog(chat_root: &Path) -> HashSet<String> {
    let changelog = chat_root
        .join("prompts")
        .join("_versions")
        .join("changelog.jsonl");
    let mut evolved = HashSet::new();
    let Ok(text) = std::fs::read_to_string(&changelog) else {
        return evolved;
    };
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(files) = v.get("files").and_then(|f| f.as_array()) {
            for f in files {
                if let Some(s) = f.as_str() {
                    evolved.insert(s.to_string());
                }
            }
        }
    }
    evolved
}

fn get_earliest_snapshot_content(chat_root: &Path, filename: &str) -> Option<String> {
    let versions_dir = chat_root.join("prompts").join("_versions");
    if !versions_dir.exists() {
        return None;
    }
    let mut txn_dirs: Vec<_> = std::fs::read_dir(&versions_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    txn_dirs.sort_by_key(|e| e.file_name());
    for txn_dir in txn_dirs {
        let file_path = txn_dir.path().join(filename);
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                return Some(content);
            }
        }
    }
    None
}

pub fn load_evolution_diffs(_workspace: &str) -> Vec<EvolutionFileDiffDto> {
    let chat_root = skilllite_core::paths::chat_root();
    let prompts_dir = chat_root.join("prompts");
    if !prompts_dir.exists() {
        return Vec::new();
    }
    let evolved_files = evolved_prompt_files_from_changelog(&chat_root);
    let mut result = Vec::new();
    for filename in EVOLUTION_PROMPT_DIFF_FILENAMES {
        let path = prompts_dir.join(filename);
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.is_empty() && !evolved_files.contains(*filename) {
            continue;
        }
        let is_evolved = evolved_files.contains(*filename);
        let original_content = if is_evolved {
            get_earliest_snapshot_content(&chat_root, filename)
        } else {
            None
        };
        result.push(EvolutionFileDiffDto {
            filename: filename.to_string(),
            evolved: is_evolved,
            content,
            original_content,
        });
    }
    result
}

#[cfg(test)]
mod evolution_prompt_version_tests {
    use super::*;

    #[test]
    fn list_txns_newest_first_and_read_roundtrip() {
        let root = std::env::temp_dir().join(format!("sl_evo_txn_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000000"),
        )
        .expect("mkdir txn0");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000000")
                .join("rules.json"),
            b"v1",
        )
        .expect("write v1");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000001"),
        )
        .expect("mkdir txn1");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000001")
                .join("rules.json"),
            b"v2",
        )
        .expect("write v2");
        std::fs::write(root.join("prompts").join("rules.json"), b"live").expect("write live");

        let list = list_prompt_snapshot_txns_at(&root, "rules.json").expect("list");
        assert_eq!(list.len(), 2);
        assert!(
            list[0].modified_unix >= list[1].modified_unix,
            "expect newest txn first"
        );
        let ids: HashSet<&str> = list.iter().map(|t| t.txn_id.as_str()).collect();
        assert!(ids.contains("evo_20260101_000001"));
        assert!(ids.contains("evo_20260101_000000"));

        assert_eq!(
            read_prompt_snapshot_version_at(&root, "rules.json", PROMPT_VERSION_CURRENT)
                .expect("cur"),
            "live"
        );
        assert_eq!(
            read_prompt_snapshot_version_at(&root, "rules.json", "evo_20260101_000000")
                .expect("t0"),
            "v1"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_disallowed_filename() {
        let root = Path::new("/");
        let r = list_prompt_snapshot_txns_at(root, "../secrets");
        assert!(r.is_err());
    }

    #[test]
    fn write_chat_prompt_rejects_bad_filename() {
        let r = write_chat_prompt_text_file("../../../etc/passwd", "x");
        assert!(r.is_err());
    }

    #[test]
    fn batch_lists_multiple_files() {
        let root = std::env::temp_dir().join(format!("sl_evo_batch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000"),
        )
        .expect("mkdir");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000")
                .join("rules.json"),
            b"{}",
        )
        .expect("write rules");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000")
                .join("examples.json"),
            b"[]",
        )
        .expect("write examples");
        let names = vec!["rules.json".to_string(), "examples.json".to_string()];
        let map = list_prompt_snapshots_batch_at(&root, &names).expect("batch");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("rules.json").map(|v| v.len()), Some(1));
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// 契约：参与 diff UI 的文件名集合必须与校验函数及 [`load_evolution_diffs`] 遍历一致（防漂移）。
#[cfg(test)]
mod evolution_prompt_diff_contract_tests {
    use super::EVOLUTION_PROMPT_DIFF_FILENAMES;
    use std::collections::HashSet;

    #[test]
    fn diff_allowlist_has_unique_entries_matching_path_guards() {
        let unique: HashSet<&str> = EVOLUTION_PROMPT_DIFF_FILENAMES.iter().copied().collect();
        assert_eq!(
            unique.len(),
            EVOLUTION_PROMPT_DIFF_FILENAMES.len(),
            "duplicate basenames in EVOLUTION_PROMPT_DIFF_FILENAMES"
        );
        for name in EVOLUTION_PROMPT_DIFF_FILENAMES {
            assert!(
                super::evolution_prompt_filename_allowed(name),
                "EVOLUTION_PROMPT_DIFF_FILENAMES contains non-allowlisted: {}",
                name
            );
        }
    }

    #[test]
    fn disallow_paths_outside_prompt_allowlist() {
        assert!(!super::evolution_prompt_filename_allowed("secrets.env"));
        assert!(!super::evolution_prompt_filename_allowed(""));
    }
}
