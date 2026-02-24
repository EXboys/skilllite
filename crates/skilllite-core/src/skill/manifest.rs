use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::skill::metadata;
use crate::skill::trust::{
    self, IntegritySignal, SignatureSignal, TrustDecision, TrustTier,
};

const MANIFEST_FILE_NAME: &str = ".skilllite-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureStatus {
    Unsigned,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillIntegrityStatus {
    Ok,
    HashChanged,
    SignatureInvalid,
    Unsigned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifestEntry {
    pub name: String,
    pub source: String,
    pub version: Option<String>,
    pub hash: String,
    pub signature_status: SignatureStatus,
    #[serde(default)]
    pub trust_tier: TrustTier,
    #[serde(default)]
    pub trust_score: u8,
    #[serde(default)]
    pub tier_reason: Vec<String>,
    #[serde(default)]
    pub tier_updated_at: Option<DateTime<Utc>>,
    pub installed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub version: u32,
    pub skills: BTreeMap<String, SkillManifestEntry>,
}

impl Default for SkillManifest {
    fn default() -> Self {
        Self {
            version: 1,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillIntegrityReport {
    pub status: SkillIntegrityStatus,
    pub current_hash: String,
    pub signature_status: SignatureStatus,
    pub entry: Option<SkillManifestEntry>,
    pub trust_tier: TrustTier,
    pub trust_score: u8,
    pub trust_decision: TrustDecision,
    pub trust_reasons: Vec<String>,
}

pub fn manifest_path(skills_dir: &Path) -> PathBuf {
    skills_dir.join(MANIFEST_FILE_NAME)
}

pub fn load_manifest(skills_dir: &Path) -> Result<SkillManifest> {
    let path = manifest_path(skills_dir);
    if !path.exists() {
        return Ok(SkillManifest::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
    let manifest: SkillManifest = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse manifest JSON: {}", path.display()))?;
    Ok(manifest)
}

pub fn save_manifest(skills_dir: &Path, manifest: &SkillManifest) -> Result<()> {
    fs::create_dir_all(skills_dir)
        .with_context(|| format!("Failed to create skills dir: {}", skills_dir.display()))?;
    let path = manifest_path(skills_dir);
    let data = serde_json::to_string_pretty(manifest)?;
    fs::write(&path, data)
        .with_context(|| format!("Failed to write manifest: {}", path.display()))?;
    Ok(())
}

pub fn upsert_installed_skill(skills_dir: &Path, skill_dir: &Path, source: &str) -> Result<SkillManifestEntry> {
    let mut manifest = load_manifest(skills_dir)?;
    let entry = build_entry(skill_dir, source)?;
    let key = skill_key(skill_dir)?;
    manifest.skills.insert(key, entry.clone());
    save_manifest(skills_dir, &manifest)?;
    Ok(entry)
}

pub fn remove_skill_entry(skills_dir: &Path, skill_dir: &Path) -> Result<bool> {
    let mut manifest = load_manifest(skills_dir)?;
    let key = skill_key(skill_dir)?;
    let removed = manifest.skills.remove(&key).is_some();
    if removed {
        save_manifest(skills_dir, &manifest)?;
    }
    Ok(removed)
}

pub fn evaluate_skill_status(skills_dir: &Path, skill_dir: &Path) -> Result<SkillIntegrityReport> {
    let manifest = load_manifest(skills_dir)?;
    let key = skill_key(skill_dir)?;
    let entry = manifest.skills.get(&key).cloned();
    let current_hash = compute_skill_fingerprint(skill_dir)?;
    let signature_status = read_signature_status(skill_dir, &current_hash)?;

    let status = if signature_status == SignatureStatus::Invalid {
        SkillIntegrityStatus::SignatureInvalid
    } else if let Some(ref item) = entry {
        if item.hash != current_hash {
            SkillIntegrityStatus::HashChanged
        } else if signature_status == SignatureStatus::Unsigned {
            SkillIntegrityStatus::Unsigned
        } else {
            SkillIntegrityStatus::Ok
        }
    } else if signature_status == SignatureStatus::Unsigned {
        SkillIntegrityStatus::Unsigned
    } else {
        // No baseline fingerprint in manifest but signed payload exists.
        // Treat as changed so execution requires an explicit re-install/update.
        SkillIntegrityStatus::HashChanged
    };

    let source = entry.as_ref().map(|e| e.source.as_str());
    let assessment = trust::assess_skill_trust(
        source,
        map_signature_signal(&signature_status),
        map_integrity_signal(&status),
        false,
        false,
    );

    Ok(SkillIntegrityReport {
        status,
        current_hash,
        signature_status,
        entry,
        trust_tier: assessment.tier,
        trust_score: assessment.score,
        trust_decision: assessment.decision,
        trust_reasons: assessment.reasons,
    })
}

fn build_entry(skill_dir: &Path, source: &str) -> Result<SkillManifestEntry> {
    let meta = metadata::parse_skill_metadata(skill_dir)?;
    let hash = compute_skill_fingerprint(skill_dir)?;
    let signature_status = read_signature_status(skill_dir, &hash)?;
    let integrity_status = match signature_status {
        SignatureStatus::Invalid => SkillIntegrityStatus::SignatureInvalid,
        SignatureStatus::Unsigned => SkillIntegrityStatus::Unsigned,
        SignatureStatus::Valid => SkillIntegrityStatus::Ok,
    };
    let assessment = trust::assess_skill_trust(
        Some(source),
        map_signature_signal(&signature_status),
        map_integrity_signal(&integrity_status),
        false,
        false,
    );
    Ok(SkillManifestEntry {
        name: if meta.name.is_empty() {
            skill_key(skill_dir)?
        } else {
            meta.name
        },
        source: source.to_string(),
        version: meta.version,
        hash,
        signature_status,
        trust_tier: assessment.tier,
        trust_score: assessment.score,
        tier_reason: assessment.reasons,
        tier_updated_at: Some(Utc::now()),
        installed_at: Utc::now(),
    })
}

fn map_signature_signal(signature_status: &SignatureStatus) -> SignatureSignal {
    match signature_status {
        SignatureStatus::Unsigned => SignatureSignal::Unsigned,
        SignatureStatus::Valid => SignatureSignal::Valid,
        SignatureStatus::Invalid => SignatureSignal::Invalid,
    }
}

fn map_integrity_signal(status: &SkillIntegrityStatus) -> IntegritySignal {
    match status {
        SkillIntegrityStatus::Ok => IntegritySignal::Ok,
        SkillIntegrityStatus::HashChanged => IntegritySignal::HashChanged,
        SkillIntegrityStatus::SignatureInvalid => IntegritySignal::SignatureInvalid,
        SkillIntegrityStatus::Unsigned => IntegritySignal::Unsigned,
    }
}

fn skill_key(skill_dir: &Path) -> Result<String> {
    skill_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Invalid skill directory: {}", skill_dir.display()))
}

fn read_signature_status(skill_dir: &Path, hash: &str) -> Result<SignatureStatus> {
    let sig_path = skill_dir.join("SKILL.sig");
    if !sig_path.exists() {
        return Ok(SignatureStatus::Unsigned);
    }

    let expected = fs::read_to_string(&sig_path)
        .with_context(|| format!("Failed to read signature file: {}", sig_path.display()))?;
    let expected = expected.trim();
    if expected.is_empty() {
        return Ok(SignatureStatus::Invalid);
    }

    if expected.eq_ignore_ascii_case(hash) {
        Ok(SignatureStatus::Valid)
    } else {
        Ok(SignatureStatus::Invalid)
    }
}

pub fn compute_skill_fingerprint(skill_dir: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(skill_dir, skill_dir, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    for rel in files {
        let file_path = skill_dir.join(&rel);
        let content = fs::read(&file_path)
            .with_context(|| format!("Failed to read file for hashing: {}", file_path.display()))?;
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update(&content);
        hasher.update([0u8]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(root: &Path, current: &Path, out: &mut Vec<String>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .with_context(|| format!("Failed to read directory: {}", current.display()))?
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let ignored_dirs: HashSet<&str> = HashSet::from([
        ".git",
        "__pycache__",
        "node_modules",
        "dist",
        "build",
        ".venv",
        "venv",
    ]);

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if ignored_dirs.contains(name.as_ref()) {
                continue;
            }
            collect_files(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if name == MANIFEST_FILE_NAME || name == ".DS_Store" {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
        out.push(rel);
    }
    Ok(())
}
