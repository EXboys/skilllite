//! Default security rules for Python and JavaScript/Node.js
//!
//! This module contains the built-in security rules that are used by default
//! when scanning scripts for security issues.

use super::rules::SecurityRule;
use super::types::{SecurityIssueType, SecuritySeverity};

/// Get default Python security rules with improved patterns to reduce false positives
pub fn get_default_python_rules() -> Vec<SecurityRule> {
    vec![
        // ========================================================================
        // File Operations
        // ========================================================================
        // Use word boundary to avoid matching method calls like `file.open()`
        SecurityRule::new(
            "py-file-open",
            r"(?:^|[^.\w])open\s*\(",
            SecurityIssueType::FileOperation,
            SecuritySeverity::Medium,
            "Built-in open() function detected (file operation)",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-file-delete",
            r"os\.(?:remove|unlink)|shutil\.rmtree",
            SecurityIssueType::FileOperation,
            SecuritySeverity::High,
            "File deletion operation",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-dir-list",
            r"os\.(?:listdir|walk)|glob\.glob|pathlib\.Path(?:\([^)]*\))?\.iterdir",
            SecurityIssueType::FileOperation,
            SecuritySeverity::Medium,
            "Directory listing operation",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Network Operations
        // ========================================================================
        SecurityRule::new(
            "py-net-import",
            r"(?:urllib|requests|http\.client|socket)\.",
            SecurityIssueType::NetworkRequest,
            SecuritySeverity::Medium,
            "Network library usage",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-net-request",
            r"(?:urlopen|requests\.(?:get|post|put|delete|patch)|socket\.connect)\s*\(",
            SecurityIssueType::NetworkRequest,
            SecuritySeverity::Medium,
            "Network request",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Code Injection - Critical Patterns
        // ========================================================================
        SecurityRule::new(
            "py-eval",
            r"(?:^|[^.\w])eval\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::Critical,
            "eval() function - arbitrary code execution",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-exec",
            r"(?:^|[^.\w])exec\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::Critical,
            "exec() function - arbitrary code execution",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-compile",
            r"(?:^|[^.\w])compile\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::High,
            "compile() function - code compilation",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-unsafe-deserialize",
            r"(?:pickle|marshal)\.loads?\s*\(|yaml\.(?:load|unsafe_load)\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::High,
            "Unsafe deserialization (potential code execution)",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Process Execution
        // ========================================================================
        SecurityRule::new(
            "py-subprocess",
            r"subprocess\.(?:call|run|Popen|check_output|check_call)\s*\(",
            SecurityIssueType::ProcessExecution,
            SecuritySeverity::High,
            "Subprocess execution",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-os-system",
            r"os\.(?:system|popen|spawn[lv]?[pe]?)\s*\(",
            SecurityIssueType::ProcessExecution,
            SecuritySeverity::Critical,
            "OS command execution",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Memory Bombs
        // ========================================================================
        SecurityRule::new(
            "py-large-array",
            r#"\[\s*(?:0|None|''|"")\s*\]\s*\*\s*\d{7,}"#,
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::High,
            "Large array allocation (potential memory bomb)",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-large-range",
            r"list\s*\(\s*range\s*\(\s*\d{8,}",
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::High,
            "Large range allocation (potential memory bomb)",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-large-bytes",
            r"(?:bytearray|bytes)\s*\(\s*\d{8,}\s*\)",
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::High,
            "Large byte allocation (potential memory bomb)",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-infinite-loop",
            r"while\s+True\s*:",
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::Medium,
            "Potential infinite loop",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Dynamic Imports
        // ========================================================================
        SecurityRule::new(
            "py-dynamic-import",
            r"__import__\s*\(|importlib\.import_module\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::Critical,
            "Dynamic import (bypasses static analysis)",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // System Information Access
        // ========================================================================
        SecurityRule::new(
            "py-env-access",
            r"os\.(?:environ|getenv|putenv)",
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Medium,
            "Environment variable access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-platform-info",
            r"platform\.(?:system|version|platform|machine|node)",
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Medium,
            "System information access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-sys-info",
            r"sys\.(?:path|modules|argv|version|executable)",
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Low,
            "Python runtime information access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-user-info",
            r"(?:pwd\.getpwuid|os\.(?:getuid|getgid|getlogin))",
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Medium,
            "User/group information access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-psutil",
            r"psutil\.(?:cpu|mem|disk|net|process|Process)",
            SecurityIssueType::SystemAccess,
            SecuritySeverity::High,
            "Process/system monitoring library",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Built-in Function Modification
        // ========================================================================
        SecurityRule::new(
            "py-builtins",
            r"__builtins__",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::High,
            "Built-in scope access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-scope-access",
            r"(?:globals|locals|vars)\s*\(\s*\)",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::High,
            "Global/local scope access",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-builtins-modify",
            r"(?:(?:setattr|delattr)\s*\(\s*(?:__builtins__|builtins)\b|builtins\.\w+\s*=[^=])",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::Critical,
            "Modification of built-in functions",
        )
        .for_languages(&["python"]),
        // ========================================================================
        // Dangerous Module Imports
        // ========================================================================
        SecurityRule::new(
            "py-ctypes-import",
            r"(?:^|[^#])\s*import\s+ctypes|from\s+ctypes\s+import",
            SecurityIssueType::DangerousModule,
            SecuritySeverity::Critical,
            "ctypes import (allows arbitrary memory access)",
        )
        .for_languages(&["python"]),
        SecurityRule::new(
            "py-os-import",
            r"(?:^|[^#])\s*import\s+(?:os|subprocess|shutil)\b",
            SecurityIssueType::DangerousModule,
            SecuritySeverity::High,
            "System module import",
        )
        .for_languages(&["python"]),
    ]
}

/// Get default JavaScript/Node.js security rules
pub fn get_default_javascript_rules() -> Vec<SecurityRule> {
    vec![
        // ========================================================================
        // Code Injection
        // ========================================================================
        SecurityRule::new(
            "js-eval",
            r"(?:^|[^.\w])eval\s*\(|new\s+Function\s*\(",
            SecurityIssueType::CodeInjection,
            SecuritySeverity::Critical,
            "eval() or Function constructor - arbitrary code execution",
        )
        .for_languages(&["javascript", "node"]),
        // ========================================================================
        // Network Operations
        // ========================================================================
        SecurityRule::new(
            "js-fetch",
            r"(?:fetch|axios|got)\s*\(",
            SecurityIssueType::NetworkRequest,
            SecuritySeverity::Medium,
            "HTTP request",
        )
        .for_languages(&["javascript", "node"]),
        SecurityRule::new(
            "js-xhr",
            r"new\s+XMLHttpRequest|https?\.request\s*\(",
            SecurityIssueType::NetworkRequest,
            SecuritySeverity::Medium,
            "HTTP request",
        )
        .for_languages(&["javascript", "node"]),
        // ========================================================================
        // File Operations (Node.js)
        // ========================================================================
        SecurityRule::new(
            "js-fs-sync",
            r"fs\.(?:readFileSync|writeFileSync|appendFileSync|unlinkSync|rmdirSync|rmSync)\s*\(",
            SecurityIssueType::FileOperation,
            SecuritySeverity::Medium,
            "Synchronous file operation",
        )
        .for_languages(&["javascript", "node"]),
        SecurityRule::new(
            "js-fs-async",
            r"fs(?:Promises)?\.(?:readFile|writeFile|appendFile|unlink|rmdir|rm)\s*\(",
            SecurityIssueType::FileOperation,
            SecuritySeverity::Medium,
            "Asynchronous file operation",
        )
        .for_languages(&["javascript", "node"]),
        // ========================================================================
        // Process Execution (Node.js)
        // ========================================================================
        SecurityRule::new(
            "js-child-process",
            r"child_process\.(?:exec|execSync|spawn|spawnSync|fork)\s*\(",
            SecurityIssueType::ProcessExecution,
            SecuritySeverity::High,
            "Child process execution",
        )
        .for_languages(&["javascript", "node"]),
        // ========================================================================
        // Memory Bombs
        // ========================================================================
        SecurityRule::new(
            "js-large-array",
            r"new\s+Array\s*\(\s*\d{6,}\s*\)|Array\s*\(\s*\d{6,}\s*\)\.fill",
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::High,
            "Large array allocation (potential memory bomb)",
        )
        .for_languages(&["javascript", "node"]),
        SecurityRule::new(
            "js-infinite-loop",
            r"while\s*\(\s*true\s*\)",
            SecurityIssueType::MemoryBomb,
            SecuritySeverity::Medium,
            "Potential infinite loop",
        )
        .for_languages(&["javascript", "node"]),
        // ========================================================================
        // Module Imports
        // ========================================================================
        SecurityRule::new(
            "js-require-child-process",
            r#"require\s*\(\s*['"]child_process['"]\s*\)"#,
            SecurityIssueType::SystemAccess,
            SecuritySeverity::High,
            "Child process module import",
        )
        .for_languages(&["javascript", "node"]),
        SecurityRule::new(
            "js-require-fs",
            r#"require\s*\(\s*['"]fs['"]\s*\)"#,
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Medium,
            "File system module import",
        )
        .for_languages(&["javascript", "node"]),
        SecurityRule::new(
            "js-import-fs",
            r#"import\s+.*\s+from\s+['"]fs['"']|import\s*\(\s*['"]fs['"]\s*\)"#,
            SecurityIssueType::SystemAccess,
            SecuritySeverity::Medium,
            "File system module import (ESM)",
        )
        .for_languages(&["javascript", "node"]),
    ]
}

/// Get all default security rules
pub fn get_default_rules() -> Vec<SecurityRule> {
    let mut rules = get_default_python_rules();
    rules.extend(get_default_javascript_rules());
    rules
}
