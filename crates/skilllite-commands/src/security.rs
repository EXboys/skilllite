//! Security commands: security scan, dependency audit.

use skilllite_core::path_validation::validate_path_under_root;
use skilllite_sandbox::security::{format_scan_result_compact, format_scan_result_json, ScriptScanner};
use anyhow::Result;

/// Perform security scan on a script.
pub fn security_scan_script(
    script_path: &str,
    allow_network: bool,
    allow_file_ops: bool,
    allow_process_exec: bool,
    json_output: bool,
) -> Result<()> {
    let path = validate_path_under_root(script_path, "Script path")?;

    let scanner = ScriptScanner::new()
        .allow_network(allow_network)
        .allow_file_ops(allow_file_ops)
        .allow_process_exec(allow_process_exec);

    let scan_result = scanner.scan_file(&path)?;

    if json_output {
        println!("{}", format_scan_result_json(&scan_result));
    } else {
        println!("Security Scan Results for: {}\n", path.display());
        println!("{}", format_scan_result_compact(&scan_result));
    }

    Ok(())
}

/// Build MetadataHint from SkillMetadata; fill resolved_packages from compatibility when missing.
#[cfg(feature = "audit")]
pub(crate) fn metadata_hint_from_skill_metadata(
    meta: &skilllite_core::skill::metadata::SkillMetadata,
) -> skilllite_sandbox::security::dependency_audit::MetadataHint {
    use skilllite_core::skill::deps;
    use skilllite_sandbox::security::dependency_audit::MetadataHint;
    let resolved_packages = meta.resolved_packages.clone().or_else(|| {
        let pkgs = deps::parse_compatibility_for_packages(meta.compatibility.as_deref());
        if pkgs.is_empty() {
            None
        } else {
            Some(pkgs)
        }
    });
    MetadataHint {
        compatibility: meta.compatibility.clone(),
        resolved_packages,
        description: meta.description.clone(),
        language: meta.language.clone(),
        entry_point: meta.entry_point.clone(),
    }
}

/// Audit skill dependencies for known vulnerabilities via OSV.dev.
///
/// Parses SKILL.md in the commands layer and passes metadata to sandbox for
/// dependency inference — sandbox never imports or parses skill metadata.
#[cfg(feature = "audit")]
pub fn dependency_audit_skill(skill_dir: &str, json_output: bool) -> Result<()> {

    let path = validate_path_under_root(skill_dir, "Skill directory")?;

    // Parse SKILL.md in commands layer; fill resolved_packages from compatibility when needed
    let metadata_hint = skilllite_core::skill::metadata::parse_skill_metadata(&path)
        .ok()
        .map(|m| metadata_hint_from_skill_metadata(&m));

    let result =
        skilllite_sandbox::security::dependency_audit::audit_skill_dependencies(&path, metadata_hint.as_ref())?;

    if json_output {
        println!(
            "{}",
            skilllite_sandbox::security::dependency_audit::format_audit_result_json(&result)
        );
    } else {
        println!(
            "{}",
            skilllite_sandbox::security::dependency_audit::format_audit_result(&result)
        );
    }

    if result.vulnerable_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}
