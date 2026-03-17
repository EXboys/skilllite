//! Human-readable and JSON formatting of audit results.

use super::types::DependencyAuditResult;

/// Format audit result for human-readable terminal display.
pub fn format_audit_result(result: &DependencyAuditResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    if !result.malicious.is_empty() {
        lines.push(format!(
            "🔴 Malicious Package Library: {} known-malicious package(s) detected!",
            result.malicious.len()
        ));
        lines.push(String::new());
        for hit in &result.malicious {
            lines.push(format!("  ☠️  {} [{}]", hit.name, hit.ecosystem));
            lines.push(format!("     └─ {}", hit.reason));
        }
        lines.push(String::new());
    }

    if result.scanned == 0 {
        if result.malicious.is_empty() {
            return "ℹ  No dependencies detected (no files, lock, or inferred packages)."
                .to_string();
        }
        return lines.join("\n");
    }

    if result.vulnerable_count == 0 && result.malicious.is_empty() {
        return format!(
            "✅ Scanned {} dependencies — no known vulnerabilities found.",
            result.scanned
        );
    }

    if result.vulnerable_count == 0 {
        lines.push(format!(
            "✅ Scanned {} dependencies — no known CVE vulnerabilities found.",
            result.scanned
        ));
        return lines.join("\n");
    }

    lines.push(format!(
        "⚠️  Supply Chain Audit: {}/{} packages have known vulnerabilities ({} total)",
        result.vulnerable_count, result.scanned, result.total_vulns
    ));
    lines.push(String::new());

    for entry in &result.entries {
        if entry.vulns.is_empty() {
            continue;
        }
        lines.push(format!(
            "  🔴 {} {} [{}]",
            entry.name, entry.version, entry.ecosystem
        ));
        for vuln in entry.vulns.iter().take(10) {
            let fix = if vuln.fixed_in.is_empty() {
                String::new()
            } else {
                format!(" → fix: {}", vuln.fixed_in.join(", "))
            };
            let summary = if vuln.summary.is_empty() {
                String::new()
            } else {
                let s = if vuln.summary.len() > 60 {
                    format!("{}...", &vuln.summary[..57])
                } else {
                    vuln.summary.clone()
                };
                format!(" — {}", s)
            };
            lines.push(format!("     └─ {}{}{}", vuln.id, summary, fix));
        }
        if entry.vulns.len() > 10 {
            lines.push(format!("     ... and {} more", entry.vulns.len() - 10));
        }
        lines.push(String::new());
    }

    let tip = match &result.backend {
        super::types::AuditBackend::Custom(url) => format!("🔗 Scanned via custom API: {}", url),
        super::types::AuditBackend::Native => {
            "💡 Visit https://osv.dev/vulnerability/<ID> for details.".to_string()
        }
    };
    lines.push(tip);

    lines.join("\n")
}

/// Format audit result as structured JSON.
pub fn format_audit_result_json(result: &DependencyAuditResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
}
