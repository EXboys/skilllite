//! SKILL.md security scanning for supply chain / agent-driven social engineering attacks.
//! Detects patterns that may instruct users to run malicious commands (e.g. ClawHavoc-style).

/// Alert for a suspicious pattern found in SKILL.md.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkillMdAlert {
    pub pattern: String,
    pub severity: String,
    pub message: String,
}

/// Scan SKILL.md content for patterns that may indicate supply chain or agent-driven social engineering.
/// Returns list of alerts (high + medium severity).
pub fn scan_skill_md_suspicious_patterns(content: &str) -> Vec<SkillMdAlert> {
    let mut alerts = Vec::new();
    let lower = content.to_lowercase();

    // High severity: direct pipe to shell
    if lower.contains("| bash") || lower.contains("|bash") {
        alerts.push(SkillMdAlert {
            pattern: "| bash".to_string(),
            severity: "high".to_string(),
            message: "SKILL.md contains '| bash' - may instruct user to run remote script"
                .to_string(),
        });
    }
    if lower.contains("| sh") && !lower.contains("#!/bin/sh") {
        alerts.push(SkillMdAlert {
            pattern: "| sh".to_string(),
            severity: "high".to_string(),
            message: "SKILL.md contains '| sh' - may instruct user to run remote script"
                .to_string(),
        });
    }
    if lower.contains("base64 -d") || lower.contains("base64 -D") {
        alerts.push(SkillMdAlert {
            pattern: "base64 -d/-D".to_string(),
            severity: "high".to_string(),
            message: "SKILL.md contains base64 decode - common in obfuscated payload delivery"
                .to_string(),
        });
    }

    // High severity: pastebin / known malicious host patterns
    if lower.contains("rentry.co") || lower.contains("pastebin.com") {
        alerts.push(SkillMdAlert {
            pattern: "pastebin/rentry".to_string(),
            severity: "high".to_string(),
            message: "SKILL.md links to pastebin/rentry - often used to host second-stage payloads"
                .to_string(),
        });
    }

    // Medium severity: instructions to run in terminal
    for (pattern, msg) in [
        ("run in terminal", "Instructions to run command in user terminal"),
        ("copy and paste", "Instructions to copy-paste command (social engineering)"),
        ("copy and run", "Instructions to copy and run command"),
        ("run this command", "Direct instruction to run a command"),
        ("execute this command", "Direct instruction to execute a command"),
        ("在终端运行", "Instructions to run in terminal (Chinese)"),
        ("复制并执行", "Instructions to copy and execute (Chinese)"),
    ] {
        if lower.contains(pattern) {
            alerts.push(SkillMdAlert {
                pattern: pattern.to_string(),
                severity: "medium".to_string(),
                message: format!("SKILL.md contains '{}' - {}", pattern, msg),
            });
        }
    }

    // Medium: curl/wget in Prerequisites/Setup context
    let in_prereq_or_setup =
        lower.contains("prerequisite") || lower.contains("setup") || lower.contains("install");
    if in_prereq_or_setup && (lower.contains("curl ") || lower.contains("wget ")) {
        alerts.push(SkillMdAlert {
            pattern: "curl/wget in prereq".to_string(),
            severity: "medium".to_string(),
            message: "SKILL.md mentions curl/wget in prerequisites/setup - verify before following"
                .to_string(),
        });
    }

    alerts
}

/// Returns true if SKILL.md content contains high-severity suspicious patterns.
pub fn has_skill_md_high_risk_patterns(content: &str) -> bool {
    scan_skill_md_suspicious_patterns(content)
        .iter()
        .any(|a| a.severity == "high")
}
