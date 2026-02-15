"""Sandbox executor for MCP code execution."""

import hashlib
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from .security import SecurityScanResult


class SandboxExecutor:
    """Secure code execution sandbox using Rust skillbox."""

    # Cache scan results for confirmation flow (scan_id -> result)
    _scan_cache: Dict[str, SecurityScanResult] = {}
    # Cache expiry time in seconds
    SCAN_CACHE_TTL = 300  # 5 minutes

    def __init__(self):
        from ...sandbox.skillbox import find_binary

        self.skillbox_path = os.getenv("SKILLBOX_PATH") or find_binary() or "./skillbox/target/release/skillbox"
        self.timeout = int(os.getenv("MCP_SANDBOX_TIMEOUT", "30"))
        self.runtime_available = os.path.exists(self.skillbox_path) and os.access(self.skillbox_path, os.X_OK)

        # Read default sandbox level from environment variable
        # SKILLBOX_SANDBOX_LEVEL: 1=no sandbox, 2=sandbox only, 3=sandbox+scan (default)
        default_level = os.getenv("SKILLBOX_SANDBOX_LEVEL", "3")
        try:
            self.default_sandbox_level = int(default_level)
            if self.default_sandbox_level not in [1, 2, 3]:
                self.default_sandbox_level = 3
        except ValueError:
            self.default_sandbox_level = 3

    def _generate_code_hash(self, language: str, code: str) -> str:
        """Generate a hash of the code for verification."""
        content = f"{language}:{code}"
        return hashlib.sha256(content.encode()).hexdigest()

    def _generate_scan_id(self, code_hash: str) -> str:
        """Generate a unique scan ID."""
        timestamp = str(time.time())
        return hashlib.sha256(f"{code_hash}:{timestamp}".encode()).hexdigest()[:16]

    def _cleanup_expired_scans(self):
        """Remove expired scan results from cache."""
        current_time = time.time()
        expired_ids = [
            scan_id for scan_id, result in self._scan_cache.items()
            if current_time - result.timestamp > self.SCAN_CACHE_TTL
        ]
        for scan_id in expired_ids:
            del self._scan_cache[scan_id]

    def _wrap_code_for_json_output(self, language: str, code: str) -> str:
        """Wrap user code to produce JSON output (required by skillbox).

        skillbox requires all skill output to be valid JSON.
        This wraps arbitrary user code to capture stdout/stderr
        and output them as a JSON object.
        """
        if language == "python":
            # Escape the code for embedding in a triple-quoted string
            # Use a unique delimiter to avoid conflicts with user code
            return f'''import json as __j__, sys as __s__, io as __io__, traceback as __tb__
__out__ = __io__.StringIO()
__err__ = __io__.StringIO()
__old_out__ = __s__.stdout
__old_err__ = __s__.stderr
__s__.stdout = __out__
__s__.stderr = __err__
__ok__ = True
try:
    exec(compile({repr(code)}, "<mcp>", "exec"))
except SystemExit:
    pass
except:
    __ok__ = False
    __err__.write(__tb__.format_exc())
finally:
    __s__.stdout = __old_out__
    __s__.stderr = __old_err__
    print(__j__.dumps({{"stdout": __out__.getvalue(), "stderr": __err__.getvalue(), "success": __ok__}}))
'''
        elif language == "javascript":
            escaped_code = code.replace('\\', '\\\\').replace('`', '\\`').replace('$', '\\$')
            return f'''const __origLog = console.log;
const __origErr = console.error;
let __stdout = [];
let __stderr = [];
console.log = (...a) => __stdout.push(a.map(String).join(" "));
console.error = (...a) => __stderr.push(a.map(String).join(" "));
let __ok = true;
try {{
    eval(`{escaped_code}`);
}} catch(e) {{
    __ok = false;
    __stderr.push(String(e.stack || e));
}} finally {{
    console.log = __origLog;
    console.error = __origErr;
    console.log(JSON.stringify({{stdout: __stdout.join("\\n"), stderr: __stderr.join("\\n"), success: __ok}}));
}}
'''
        elif language == "bash":
            escaped_code = code.replace("'", "'\\''")
            return f'''__out__=$(eval '{escaped_code}' 2>&1) ; __rc__=$?
if [ $__rc__ -eq 0 ]; then __ok__=true; else __ok__=false; fi
printf '{{"stdout": "%s", "stderr": "", "success": %s}}' "$(echo "$__out__" | sed 's/"/\\\\"/g' | tr '\\n' ' ')" "$__ok__"
'''
        else:
            return code

    def _create_temp_skill(self, language: str, code: str, wrap_json: bool = False) -> Tuple[str, str]:
        """Create a temporary skill directory with the code file.

        Args:
            language: Programming language
            code: User code to execute
            wrap_json: If True, wrap the code to produce JSON output (for execute_code)
        """
        skill_dir = tempfile.mkdtemp(prefix="mcp_skill_")

        # Create scripts subdirectory (required by skillbox)
        scripts_dir = os.path.join(skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)

        ext = self.get_file_extension(language)
        entry_point = f"scripts/main.{ext}"

        final_code = self._wrap_code_for_json_output(language, code) if wrap_json else code

        skill_md_content = f"""---
name: mcp-execution
entry_point: {entry_point}
language: {language}
description: MCP code execution skill
network:
  enabled: true
---

This skill executes code from MCP.
"""
        with open(os.path.join(skill_dir, "SKILL.md"), "w") as f:
            f.write(skill_md_content)

        code_file = os.path.join(scripts_dir, f"main.{ext}")
        with open(code_file, "w") as f:
            f.write(final_code)
        os.chmod(code_file, 0o755)

        return skill_dir, code_file

    def scan_code(self, language: str, code: str, sandbox_level: Optional[int] = None) -> SecurityScanResult:
        """Scan code for security issues without executing it.

        Args:
            language: Programming language (python, javascript, bash)
            code: Code to scan
            sandbox_level: Sandbox level to check against (default: from env or 3)
        """
        # Use default sandbox level if not specified
        if sandbox_level is None:
            sandbox_level = self.default_sandbox_level

        if not self.runtime_available:
            return SecurityScanResult(
                is_safe=False,
                issues=[{"severity": "Critical", "issue_type": "RuntimeError",
                        "description": f"skillbox not found at {self.skillbox_path}",
                        "rule_id": "system", "line_number": 0, "code_snippet": ""}],
                scan_id="error",
                code_hash="",
                high_severity_count=1,
                sandbox_level=sandbox_level,
            )

        self._cleanup_expired_scans()
        code_hash = self._generate_code_hash(language, code)
        scan_id = self._generate_scan_id(code_hash)

        try:
            skill_dir, code_file = self._create_temp_skill(language, code)

            try:
                scan_env = os.environ.copy()
                scan_env["SKILLBOX_SKILLS_ROOT"] = os.path.dirname(skill_dir)

                result = subprocess.run(
                    [self.skillbox_path, "security-scan", "--json", code_file],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    env=scan_env,
                )

                # Parse structured JSON output
                from skilllite.core.security import parse_scan_json_output
                data = parse_scan_json_output(result.stdout)

                scan_result = SecurityScanResult(
                    is_safe=data["is_safe"],
                    issues=data["issues"],
                    scan_id=scan_id,
                    code_hash=code_hash,
                    high_severity_count=data["high_severity_count"],
                    medium_severity_count=data["medium_severity_count"],
                    low_severity_count=data["low_severity_count"],
                    sandbox_level=sandbox_level,
                )

                self._scan_cache[scan_id] = scan_result
                return scan_result

            finally:
                shutil.rmtree(skill_dir, ignore_errors=True)

        except subprocess.TimeoutExpired:
            return SecurityScanResult(
                is_safe=False,
                issues=[{"severity": "Critical", "issue_type": "Timeout",
                        "description": "Security scan timed out",
                        "rule_id": "system", "line_number": 0, "code_snippet": ""}],
                scan_id=scan_id,
                code_hash=code_hash,
                high_severity_count=1,
                sandbox_level=sandbox_level,
            )
        except Exception as e:
            return SecurityScanResult(
                is_safe=False,
                issues=[{"severity": "Critical", "issue_type": "ScanError",
                        "description": str(e),
                        "rule_id": "system", "line_number": 0, "code_snippet": ""}],
                scan_id=scan_id,
                code_hash=code_hash,
                high_severity_count=1,
                sandbox_level=sandbox_level,
            )

    def verify_scan(self, scan_id: str, code_hash: str) -> Optional[SecurityScanResult]:
        """Verify a scan result exists and matches the code hash."""
        self._cleanup_expired_scans()

        if scan_id not in self._scan_cache:
            return None

        result = self._scan_cache[scan_id]
        if result.code_hash != code_hash:
            return None

        return result

    def execute(
        self,
        language: str,
        code: str,
        confirmed: bool = False,
        scan_id: Optional[str] = None,
        sandbox_level: Optional[int] = None,
    ) -> Dict[str, Any]:
        """Execute code in a secure sandbox using Rust skillbox.

        Args:
            language: Programming language (python, javascript, bash)
            code: Code to execute
            confirmed: Whether user has confirmed execution despite security warnings
            scan_id: Scan ID from previous scan_code call (required when confirmed=True)
            sandbox_level: Override sandbox level (default: from SKILLBOX_SANDBOX_LEVEL env or 3)
        """
        # Use default sandbox level from environment if not specified
        if sandbox_level is None:
            sandbox_level = self.default_sandbox_level
        if not self.runtime_available:
            return {
                "success": False,
                "stdout": "",
                "stderr": f"skillbox not found at {self.skillbox_path}. Please build it with: cd skillbox && cargo build --release",
                "exit_code": 1
            }

        code_hash = self._generate_code_hash(language, code)

        if sandbox_level >= 3 and not confirmed:
            scan_result = self.scan_code(language, code, sandbox_level=sandbox_level)

            # Check for hard blocked issues first
            if scan_result.has_hard_blocked:
                return {
                    "success": False,
                    "stdout": "",
                    "stderr": (
                        f"🚫 Execution Blocked\n\n"
                        f"{scan_result.format_report()}\n\n"
                        f"❌ This code contains operations that are PERMANENTLY BLOCKED\n"
                        f"   in the L{sandbox_level} sandbox environment.\n\n"
                        f"   Even with confirmation, this code CANNOT be executed.\n\n"
                        f"Options:\n"
                        f"  1. Modify the code to remove blocked operations\n"
                        f"  2. Use sandbox_level=1 or sandbox_level=2 (if permitted)\n"
                    ),
                    "exit_code": 4,
                    "hard_blocked": True,
                    "security_issues": scan_result.to_dict(),
                }

            # Soft risk: can be confirmed
            if scan_result.high_severity_count > 0:
                return {
                    "success": False,
                    "stdout": "",
                    "stderr": (
                        f"🔐 Security Review Required\n\n"
                        f"{scan_result.format_report()}\n\n"
                        f"⚠️ IMPORTANT: You MUST ask the user for confirmation before proceeding.\n"
                        f"Show this security report to the user and wait for their explicit approval.\n\n"
                        f"If the user approves, call execute_code again with:\n"
                        f"  - confirmed: true\n"
                        f"  - scan_id: \"{scan_result.scan_id}\"\n"
                    ),
                    "exit_code": 2,
                    "requires_confirmation": True,
                    "scan_id": scan_result.scan_id,
                    "security_issues": scan_result.to_dict(),
                }

        if confirmed and scan_id:
            cached_result = self.verify_scan(scan_id, code_hash)
            if not cached_result:
                return {
                    "success": False,
                    "stdout": "",
                    "stderr": (
                        "❌ Invalid or expired scan_id. The code may have been modified.\n"
                        "Please run scan_code again to get a new scan_id."
                    ),
                    "exit_code": 3,
                }

            # Even with confirmation, check for hard blocked issues
            if cached_result.has_hard_blocked:
                return {
                    "success": False,
                    "stdout": "",
                    "stderr": (
                        f"🚫 Execution Blocked (Even After Confirmation)\n\n"
                        f"The code contains operations that are PERMANENTLY BLOCKED\n"
                        f"in the L{sandbox_level} sandbox environment:\n\n"
                        + "\n".join(f"  • {issue.get('issue_type', 'Unknown')}: {issue.get('description', '')}"
                                   for issue in cached_result.hard_blocked_issues) +
                        f"\n\n"
                        f"❌ Confirmation cannot override sandbox runtime restrictions.\n\n"
                        f"Options:\n"
                        f"  1. Modify the code to remove blocked operations\n"
                        f"  2. Use sandbox_level=1 or sandbox_level=2 (if permitted)\n"
                    ),
                    "exit_code": 4,
                    "hard_blocked": True,
                    "security_issues": cached_result.to_dict(),
                }

        try:
            # Wrap user code to produce JSON output (skillbox requires JSON stdout)
            skill_dir, _ = self._create_temp_skill(language, code, wrap_json=True)

            try:
                env = os.environ.copy()
                env["SKILLBOX_AUTO_APPROVE"] = "true"
                # Allow skillbox to access temp skill directories
                env["SKILLBOX_SKILLS_ROOT"] = os.path.dirname(skill_dir)

                cmd = [self.skillbox_path, "run"]
                if sandbox_level in [1, 2, 3]:
                    cmd.extend(["--sandbox-level", str(sandbox_level)])
                cmd.extend([skill_dir, "{}"])

                result = subprocess.run(
                    cmd,
                    capture_output=True,
                    text=True,
                    timeout=self.timeout,
                    env=env,
                )

                # Try to parse the JSON output from our wrapper
                if result.returncode == 0 and result.stdout.strip():
                    try:
                        import json
                        parsed = json.loads(result.stdout.strip())
                        return {
                            "success": parsed.get("success", True),
                            "stdout": parsed.get("stdout", ""),
                            "stderr": parsed.get("stderr", ""),
                            "exit_code": 0 if parsed.get("success", True) else 1
                        }
                    except (json.JSONDecodeError, KeyError):
                        pass

                return {
                    "success": result.returncode == 0,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "exit_code": result.returncode
                }
            finally:
                shutil.rmtree(skill_dir, ignore_errors=True)

        except subprocess.TimeoutExpired:
            return {
                "success": False,
                "stdout": "",
                "stderr": f"Execution timed out after {self.timeout} seconds",
                "exit_code": 124
            }
        except Exception as e:
            return {
                "success": False,
                "stdout": "",
                "stderr": str(e),
                "exit_code": 1
            }

    def get_file_extension(self, language: str) -> str:
        """Get file extension for the given language."""
        extensions = {
            "python": "py",
            "javascript": "js",
            "bash": "sh"
        }
        return extensions.get(language, "py")
