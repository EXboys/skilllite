//! Source parsing: URL detection, local ZIP extraction, ClawHub download, git clone.

use anyhow::Context;
use regex::Regex;
use std::fs;
#[cfg(feature = "audit")]
use std::io::Cursor;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use crate::error::bail;
use crate::Result;

// ─── Static regexes (compiled once at first use) ──────────────────────────────

/// Fallback regex that never matches (used when a static pattern fails to compile).
/// Uses `$^` (end then start) which is valid and matches no string.
fn never_match_regex() -> Regex {
    Regex::new("$^").unwrap_or_else(|_| unreachable!("$^ is valid"))
}

static RE_TREE_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)")
        .unwrap_or_else(|_| never_match_regex())
});
static RE_TREE_BRANCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)$").unwrap_or_else(|_| never_match_regex())
});
static RE_GITHUB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"github\.com/([^/]+)/([^/]+?)(?:\.git)?/*$").unwrap_or_else(|_| never_match_regex())
});
static RE_GITLAB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"gitlab\.com/(.+?)(?:\.git)?/?$").unwrap_or_else(|_| never_match_regex())
});
static RE_AT_FILTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^/]+)/([^/@]+)@(.+)$").unwrap_or_else(|_| never_match_regex())
});
static RE_SHORTHAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([^/]+)/([^/]+)(?:/(.+))?$").unwrap_or_else(|_| never_match_regex())
});

// ─── Source Parsing ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub(super) struct ParsedSource {
    pub(super) source_type: String,
    pub(super) url: String,
    pub(super) git_ref: Option<String>,
    pub(super) subpath: Option<String>,
    pub(super) skill_filter: Option<String>,
}

fn is_local_path(source: &str) -> bool {
    Path::new(source).is_absolute()
        || source.starts_with("./")
        || source.starts_with("../")
        || source == "."
        || source == ".."
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

pub(super) fn parse_source(source: &str) -> ParsedSource {
    if let Some(slug) = source.strip_prefix("clawhub:") {
        let slug = slug.trim().to_lowercase();
        if !slug.is_empty() {
            return ParsedSource {
                source_type: "clawhub".into(),
                url: slug,
                git_ref: None,
                subpath: None,
                skill_filter: None,
            };
        }
    }

    if is_local_path(source) {
        let abs = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(source);
        return ParsedSource {
            source_type: if is_zip_path(&abs) {
                "local_zip".into()
            } else {
                "local".into()
            },
            url: abs.to_string_lossy().into(),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    if let Some(cap) = RE_TREE_PATH.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: Some(cap[4].to_string()),
            skill_filter: None,
        };
    }

    if let Some(cap) = RE_TREE_BRANCH.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: None,
            skill_filter: None,
        };
    }

    if let Some(cap) = RE_GITHUB.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    if let Some(cap) = RE_GITLAB.captures(source) {
        let repo_path = &cap[1];
        if repo_path.contains('/') {
            return ParsedSource {
                source_type: "gitlab".into(),
                url: format!("https://gitlab.com/{}.git", repo_path),
                git_ref: None,
                subpath: None,
                skill_filter: None,
            };
        }
    }

    if let Some(cap) = RE_AT_FILTER.captures(source) {
        if !source.contains(':') {
            return ParsedSource {
                source_type: "github".into(),
                url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
                git_ref: None,
                subpath: None,
                skill_filter: Some(cap[3].to_string()),
            };
        }
    }

    if let Some(cap) = RE_SHORTHAND.captures(source) {
        if !source.contains(':') && !source.starts_with('.') {
            return ParsedSource {
                source_type: "github".into(),
                url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
                git_ref: None,
                subpath: cap.get(3).map(|m| m.as_str().to_string()),
                skill_filter: None,
            };
        }
    }

    ParsedSource {
        source_type: "git".into(),
        url: source.to_string(),
        git_ref: None,
        subpath: None,
        skill_filter: None,
    }
}

// ─── ClawHub Download ──────────────────────────────────────────────────────

#[cfg(feature = "audit")]
const CLAWHUB_DOWNLOAD_URL: &str = "https://clawhub.ai/api/v1/download";

fn extract_zip_archive<R: Read + Seek>(reader: R, source_label: &str) -> Result<PathBuf> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    #[allow(deprecated)]
    let extract_path = temp_dir.into_path();

    let mut archive =
        zip::ZipArchive::new(reader).with_context(|| format!("Invalid zip from {source_label}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let name = file.name().to_string();
        let enclosed = file.enclosed_name().ok_or_else(|| {
            crate::Error::validation(format!(
                "ZIP entry escapes extraction root: {} ({})",
                name, source_label
            ))
        })?;
        let out_path = extract_path.join(enclosed);
        if file.is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("Failed to create {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }
            let mut out_file = fs::File::create(&out_path)
                .with_context(|| format!("Failed to create {}", out_path.display()))?;
            std::io::copy(&mut file, &mut out_file)
                .with_context(|| format!("Failed to extract {}", name))?;
        }
    }

    Ok(extract_path)
}

pub(super) fn extract_local_zip(zip_path: &Path) -> Result<PathBuf> {
    let zip_path = if zip_path.is_absolute() {
        zip_path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(crate::Error::from)?
            .join(zip_path)
    };
    let file = fs::File::open(&zip_path)
        .with_context(|| format!("Failed to open local zip: {}", zip_path.display()))?;
    extract_zip_archive(file, &zip_path.display().to_string())
}

#[cfg(feature = "audit")]
pub(super) fn fetch_from_clawhub(slug: &str) -> Result<PathBuf> {
    let url = format!("{}?slug={}", CLAWHUB_DOWNLOAD_URL, slug);
    let agent = ureq::AgentBuilder::new().build();
    let resp = agent
        .get(&url)
        .call()
        .context("Failed to fetch from ClawHub. Check network.")?;

    let status = resp.status();
    if status != 200 {
        let body = resp.into_string().unwrap_or_default();
        bail!(
            "ClawHub returned {} for slug '{}'. {}",
            status,
            slug,
            if body.len() > 200 { "" } else { &body }
        );
    }

    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .context("Failed to read zip from ClawHub")?;

    extract_zip_archive(Cursor::new(bytes), &format!("ClawHub slug {}", slug))
}

#[cfg(not(feature = "audit"))]
pub(super) fn fetch_from_clawhub(_slug: &str) -> Result<PathBuf> {
    bail!("ClawHub download requires the 'audit' feature (ureq).")
}

// ─── Git Clone ──────────────────────────────────────────────────────────────

pub(super) fn clone_repo(url: &str, git_ref: Option<&str>) -> Result<PathBuf> {
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    #[allow(deprecated)]
    let temp_path = temp_dir.into_path();

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--depth", "1"]);
    if let Some(r) = git_ref {
        cmd.args(["--branch", r]);
    }
    cmd.arg(url).arg(&temp_path);

    let output = cmd
        .output()
        .context("Failed to execute git clone. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&temp_path);
        if stderr.contains("Authentication failed") || stderr.contains("Permission denied") {
            bail!(
                "Authentication failed for {}.\n  For private repos, ensure you have access.\n  For SSH: ssh -T git@github.com\n  For HTTPS: gh auth login",
                url
            );
        }
        bail!("Failed to clone {}: {}", url, stderr.trim());
    }

    Ok(temp_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_zip(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().expect("temp zip");
        let writer = file.reopen().expect("reopen zip writer");
        let mut zip = zip::ZipWriter::new(writer);
        let options = zip::write::FileOptions::default();
        for (name, bytes) in entries {
            zip.start_file(*name, options).expect("start file");
            zip.write_all(bytes).expect("write zip file");
        }
        zip.finish().expect("finish zip");
        file
    }

    #[test]
    fn parse_source_marks_local_zip_paths() {
        let parsed = parse_source("./fixtures/sample-skill.zip");
        assert_eq!(parsed.source_type, "local_zip");
        assert!(parsed.url.ends_with("fixtures/sample-skill.zip"));
    }

    #[test]
    fn extract_local_zip_rejects_path_traversal() {
        let zip = write_zip(&[("../escape.txt", b"boom")]);
        let err = extract_local_zip(zip.path()).expect_err("zip traversal should fail");
        let msg = err.to_string();
        assert!(msg.contains("ZIP entry escapes extraction root"));
    }
}
