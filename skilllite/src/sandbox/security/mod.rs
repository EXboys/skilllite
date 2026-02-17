//! Security module for skilllite
//!
//! Three complementary layers:
//!
//! - **Static scanning** (scanner, rules, default_rules): Pre-execution analysis
//!   of script source to detect dangerous patterns (eval, subprocess, etc.)
//!
//! - **Supply chain audit** (dependency_audit, feature `audit`): Checks
//!   dependencies declared in requirements.txt / package.json against the
//!   OSV.dev vulnerability database.
//!
//! - **Runtime policy** (policy): Sandbox isolation rules — deny paths, process
//!   denylist, network policy — translated by macOS/Linux into Seatbelt/bwrap
//!
//! Submodules:
//! - **types**: Core type definitions (SecurityIssue, SecuritySeverity, etc.)
//! - **rules**: Rule definitions and configuration loading
//! - **default_rules**: Built-in security rules for Python and JavaScript
//! - **scanner**: The main ScriptScanner implementation
//! - **dependency_audit**: Supply chain vulnerability scanning via OSV API
//! - **policy**: Canonical sandbox runtime policy (paths, processes, network)
//!
//! # Example
//!
//! ```rust,ignore
//! use skilllite::sandbox::security::{ScriptScanner, format_scan_result};
//! use std::path::Path;
//!
//! let scanner = ScriptScanner::new()
//!     .allow_network(true)  // Allow network operations
//!     .disable_rules(&["py-file-open"]);  // Disable specific rules
//!
//! let result = scanner.scan_file(Path::new("script.py"))?;
//! println!("{}", format_scan_result(&result));
//! ```
//!
//! # Custom Rules Configuration
//!
//! Create a `.skilllite-rules.yaml` file in your skill directory:
//!
//! ```yaml
//! use_default_rules: true
//! disabled_rules:
//!   - py-file-open
//! rules:
//!   - id: custom-rule
//!     pattern: "dangerous_function\\s*\\("
//!     issue_type: code_injection
//!     severity: high
//!     description: "Custom dangerous function"
//!     languages: ["python"]
//! ```

pub mod default_rules;
#[cfg(feature = "audit")]
pub mod dependency_audit;
pub mod policy;
pub mod rules;
pub mod scanner;
pub mod types;

// Re-export commonly used items for public API
// These exports are intentionally kept for library users even if not used internally
#[allow(unused_imports)]
pub use default_rules::{get_default_javascript_rules, get_default_python_rules, get_default_rules};
#[allow(unused_imports)]
pub use rules::{RulesConfig, SecurityRule, CONFIG_FILE_NAMES};
pub use scanner::{format_scan_result, format_scan_result_json, ScriptScanner};
#[allow(unused_imports)]
pub use types::{ScanResult, SecurityIssue, SecurityIssueType, SecuritySeverity};
