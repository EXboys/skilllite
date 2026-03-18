//! Dependency file parsers.

use std::path::Path;

use super::types::Dependency;

/// Parse Python `requirements.txt` / `pip freeze` output.
pub fn parse_requirements_txt(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let line = line.split('#').next().unwrap_or(line).trim();

        if let Some((name, version)) = line.split_once("==") {
            push_if_valid(&mut deps, name, version, "PyPI");
            continue;
        }
        if let Some(idx) = line.find(['>', '<', '~', '!']) {
            let name = &line[..idx];
            let rest = &line[idx..];
            let version = rest.trim_start_matches(['>', '<', '~', '!', '=']);
            let version = version.split(',').next().unwrap_or("").trim();
            push_if_valid(&mut deps, name, version, "PyPI");
        }
    }
    deps
}

/// Parse Node.js `package.json` dependencies.
pub fn parse_package_json(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return deps,
    };

    for section in &["dependencies", "devDependencies"] {
        if let Some(obj) = parsed.get(section).and_then(|v| v.as_object()) {
            for (name, version_val) in obj {
                if let Some(version_str) = version_val.as_str() {
                    let version = version_str
                        .trim_start_matches('^')
                        .trim_start_matches('~')
                        .trim_start_matches(">=")
                        .trim_start_matches('>')
                        .trim_start_matches("<=")
                        .trim_start_matches('<')
                        .trim_start_matches('=')
                        .trim();
                    if version.is_empty()
                        || version.starts_with("http")
                        || version.starts_with("git")
                        || version.contains('/')
                        || version == "*"
                        || version == "latest"
                    {
                        continue;
                    }
                    deps.push(Dependency {
                        name: name.clone(),
                        version: version.to_string(),
                        ecosystem: "npm".to_string(),
                    });
                }
            }
        }
    }
    deps
}

/// Parse `.skilllite.lock` JSON for resolved packages.
pub(super) fn parse_lock_file(lock_path: &Path) -> Option<Vec<Dependency>> {
    let content = std::fs::read_to_string(lock_path).ok()?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;
    let arr = lock.get("resolved_packages")?.as_array()?;

    let packages: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if packages.is_empty() {
        return None;
    }

    let fake_requirements = packages.join("\n");
    let deps = parse_requirements_txt(&fake_requirements);

    if deps.is_empty() {
        None
    } else {
        Some(deps)
    }
}

pub(super) fn push_if_valid(
    deps: &mut Vec<Dependency>,
    name: &str,
    version: &str,
    ecosystem: &str,
) {
    let name = name.trim();
    let version = version.trim();
    if !name.is_empty() && !version.is_empty() {
        deps.push(Dependency {
            name: name.to_string(),
            version: version.to_string(),
            ecosystem: ecosystem.to_string(),
        });
    }
}
