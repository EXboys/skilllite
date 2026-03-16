//! Dependency audit tests.

#[cfg(test)]
mod tests {
    use super::super::audit::build_result;
    use super::super::config::get_custom_api;
    use super::super::parsers::{parse_package_json, parse_requirements_txt};
    use super::super::types::{AuditBackend, PackageAuditEntry, VulnRef};

    #[test]
    fn test_parse_requirements_txt_exact() {
        let content = "requests==2.31.0\nflask==3.0.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "requests");
        assert_eq!(deps[0].version, "2.31.0");
        assert_eq!(deps[0].ecosystem, "PyPI");
        assert_eq!(deps[1].name, "flask");
        assert_eq!(deps[1].version, "3.0.0");
    }

    #[test]
    fn test_parse_requirements_txt_operators() {
        let content = "requests>=2.25.0\nflask~=2.0\nnumpy<2.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].version, "2.25.0");
        assert_eq!(deps[1].version, "2.0");
        assert_eq!(deps[2].version, "2.0");
    }

    #[test]
    fn test_parse_requirements_txt_skip_comments_and_flags() {
        let content = "# comment\n-r other.txt\n-e git+https://...\nrequests==1.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "requests");
    }

    #[test]
    fn test_parse_requirements_txt_inline_comment() {
        let content = "requests==2.31.0  # HTTP library\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version, "2.31.0");
    }

    #[test]
    fn test_parse_package_json() {
        let content = r#"{
            "dependencies": {
                "express": "^4.18.2",
                "lodash": "~4.17.21"
            },
            "devDependencies": {
                "jest": ">=29.0.0"
            }
        }"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].ecosystem, "npm");
        assert!(deps.iter().any(|d| d.name == "express" && d.version == "4.18.2"));
        assert!(deps.iter().any(|d| d.name == "lodash" && d.version == "4.17.21"));
        assert!(deps.iter().any(|d| d.name == "jest" && d.version == "29.0.0"));
    }

    #[test]
    fn test_parse_package_json_skip_non_versions() {
        let content = r#"{
            "dependencies": {
                "my-lib": "git+https://github.com/foo/bar",
                "other": "*",
                "valid": "1.0.0"
            }
        }"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "valid");
    }

    #[test]
    fn test_backend_priority_custom_api() {
        assert!(get_custom_api().is_none());
    }

    #[test]
    fn test_build_result_counts() {
        let entries = vec![
            PackageAuditEntry {
                name: "a".into(),
                version: "1.0".into(),
                ecosystem: "PyPI".into(),
                vulns: vec![VulnRef {
                    id: "V-1".into(),
                    summary: "test".into(),
                    fixed_in: vec!["1.1".into()],
                }],
            },
            PackageAuditEntry {
                name: "b".into(),
                version: "2.0".into(),
                ecosystem: "npm".into(),
                vulns: vec![],
            },
        ];
        let result = build_result(entries, AuditBackend::Native, vec![]);
        assert_eq!(result.scanned, 2);
        assert_eq!(result.vulnerable_count, 1);
        assert_eq!(result.total_vulns, 1);
        assert!(result.malicious.is_empty());
    }
}
