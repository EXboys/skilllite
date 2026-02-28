//! Source parsing: URL detection, ClawHub download, git clone.

use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
            source_type: "local".into(),
            url: abs.to_string_lossy().into(),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    let re_tree_path = Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)/(.+)")
        .expect("static regex re_tree_path");
    if let Some(cap) = re_tree_path.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: Some(cap[4].to_string()),
            skill_filter: None,
        };
    }

    let re_tree_branch = Regex::new(r"github\.com/([^/]+)/([^/]+)/tree/([^/]+)$")
        .expect("static regex re_tree_branch");
    if let Some(cap) = re_tree_branch.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: Some(cap[3].to_string()),
            subpath: None,
            skill_filter: None,
        };
    }

    let re_github = Regex::new(r"github\.com/([^/]+)/([^/]+?)(?:\.git)?/*$")
        .expect("static regex re_github");
    if let Some(cap) = re_github.captures(source) {
        return ParsedSource {
            source_type: "github".into(),
            url: format!("https://github.com/{}/{}.git", &cap[1], &cap[2]),
            git_ref: None,
            subpath: None,
            skill_filter: None,
        };
    }

    let re_gitlab = Regex::new(r"gitlab\.com/(.+?)(?:\.git)?/?$")
        .expect("static regex re_gitlab");
    if let Some(cap) = re_gitlab.captures(source) {
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

    let re_at_filter = Regex::new(r"^([^/]+)/([^/@]+)@(.+)$")
        .expect("static regex re_at_filter");
    if let Some(cap) = re_at_filter.captures(source) {
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

    let re_shorthand = Regex::new(r"^([^/]+)/([^/]+)(?:/(.+))?$")
        .expect("static regex re_shorthand");
    if let Some(cap) = re_shorthand.captures(source) {
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


const CLAWHUB_DOWNLOAD_URL: &str = "https://clawhub.ai/api/v1/download";

#[cfg(feature = "audit")]
pub(super) fn fetch_from_clawhub(slug: &str) -> Result<PathBuf> {
    use std::io::Read;

    let url = format!("{}?slug={}", CLAWHUB_DOWNLOAD_URL, slug);
    let agent = ureq::AgentBuilder::new().build();
    let resp = agent
        .get(&url)
        .call()
        .context("Failed to fetch from ClawHub. Check network.")?;

    let status = resp.status();
    if status != 200 {
        let body = resp.into_string().unwrap_or_default();
        anyhow::bail!(
            "ClawHub returned {} for slug '{}'. {}",
            status,
            slug,
            if body.len() > 200 { "" } else { &body }
        );
    }

    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).context("Failed to read zip from ClawHub")?;

    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    #[allow(deprecated)]
    let extract_path = temp_dir.into_path();

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("Invalid zip from ClawHub")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;
        let name = file.name().to_string();
        if name.contains("..") || name.starts_with('/') {
            continue;
        }
        let out_path = extract_path.join(&name);
        if file.is_dir() {
            let _ = fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let mut out_file =
                fs::File::create(&out_path).with_context(|| format!("Failed to create {}", out_path.display()))?;
            std::io::copy(&mut file, &mut out_file)
                .with_context(|| format!("Failed to extract {}", name))?;
        }
    }

    Ok(extract_path)
}

#[cfg(not(feature = "audit"))]
pub(super) fn fetch_from_clawhub(_slug: &str) -> Result<PathBuf> {
    anyhow::bail!("ClawHub download requires the 'audit' feature (ureq).")
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

    let output = cmd.output().context("Failed to execute git clone. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_dir_all(&temp_path);
        if stderr.contains("Authentication failed") || stderr.contains("Permission denied") {
            anyhow::bail!(
                "Authentication failed for {}.\n  For private repos, ensure you have access.\n  For SSH: ssh -T git@github.com\n  For HTTPS: gh auth login",
                url
            );
        }
        anyhow::bail!("Failed to clone {}: {}", url, stderr.trim());
    }

    Ok(temp_path)
}
