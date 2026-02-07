"""
MCP Server implementation for SkillLite.

This module provides a secure code execution sandbox server implementing
the Model Context Protocol (MCP).

Features:
    1. **Skills Management**: List, inspect, and execute pre-defined skills
       from the skills directory.

    2. **Code Execution**: Execute arbitrary code in a secure sandbox with
       security scanning.

Security Model:
    The MCP server implements a two-phase execution model for security:

    1. **scan_code**: First, scan the code for security issues. This returns
       a detailed report of any potential risks found in the code.

    2. **execute_code**: Then, execute the code. If security issues were found,
       the caller must explicitly set `confirmed=true` to proceed.

    This allows LLM clients to present security warnings to users and get
    explicit confirmation before executing potentially dangerous code.

Environment Variables:
    SKILLBOX_SANDBOX_LEVEL: Default sandbox level (1/2/3, default: 3)
    SKILLBOX_PATH: Path to skillbox binary
    MCP_SANDBOX_TIMEOUT: Execution timeout in seconds (default: 30)
    SKILLLITE_SKILLS_DIR: Directory containing skills (default: ./.skills)
"""

import asyncio
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


def _load_dotenv():
    """Load environment variables from .env file if it exists."""
    # Try to find .env file in current directory or parent directories
    current_dir = Path.cwd()
    
    for search_dir in [current_dir] + list(current_dir.parents)[:3]:
        env_file = search_dir / ".env"
        if env_file.exists():
            try:
                with open(env_file, "r") as f:
                    for line in f:
                        line = line.strip()
                        # Skip comments and empty lines
                        if not line or line.startswith("#"):
                            continue
                        # Parse KEY=VALUE
                        if "=" in line:
                            key, _, value = line.partition("=")
                            key = key.strip()
                            value = value.strip()
                            # Remove quotes if present
                            if value and value[0] in ('"', "'") and value[-1] == value[0]:
                                value = value[1:-1]
                            # Only set if not already in environment
                            if key and key not in os.environ:
                                os.environ[key] = value
                return True
            except Exception:
                pass
    return False


# Load .env file on module import
_load_dotenv()

try:
    from mcp.server import Server
    from mcp.server.stdio import stdio_server
    from mcp.types import (
        TextContent,
        Tool,
        CallToolResult,
    )
    MCP_AVAILABLE = True
except ImportError:
    MCP_AVAILABLE = False
    # Define stub types for when MCP is not available
    Server = None  # type: ignore
    stdio_server = None  # type: ignore
    TextContent = None  # type: ignore
    Tool = None  # type: ignore
    CallToolResult = None  # type: ignore


class SecurityScanResult:
    """Result of a security scan."""

    # Issue types that are HARD BLOCKED in L3 sandbox (cannot execute even with confirmation)
    # These operations are blocked at the sandbox runtime level, not just static analysis
    HARD_BLOCKED_ISSUE_TYPES_L3 = {
        "Process Execution",   # os.system, subprocess, etc.
        "ProcessExecution",    # Alternative format
        "process_execution",   # Snake case format
    }

    # Rule IDs that are specifically hard blocked in L3 sandbox
    HARD_BLOCKED_RULE_IDS_L3 = {
        "py-subprocess",       # subprocess.call/run/Popen
        "py-os-system",        # os.system/popen/spawn
        "js-child-process",    # child_process.exec/spawn
    }

    # Dangerous module imports that lead to hard blocks when combined with execution
    HARD_BLOCKED_MODULES_L3 = {
        "py-os-import",        # import os/subprocess/shutil
    }

    def __init__(
        self,
        is_safe: bool,
        issues: List[Dict[str, Any]],
        scan_id: str,
        code_hash: str,
        high_severity_count: int = 0,
        medium_severity_count: int = 0,
        low_severity_count: int = 0,
        sandbox_level: int = 3,
    ):
        self.is_safe = is_safe
        self.issues = issues
        self.scan_id = scan_id
        self.code_hash = code_hash
        self.high_severity_count = high_severity_count
        self.medium_severity_count = medium_severity_count
        self.low_severity_count = low_severity_count
        self.sandbox_level = sandbox_level
        self.timestamp = time.time()

        # Calculate hard blocked issues
        self.hard_blocked_issues = self._find_hard_blocked_issues()
        self.has_hard_blocked = len(self.hard_blocked_issues) > 0

    def _find_hard_blocked_issues(self) -> List[Dict[str, Any]]:
        """Find issues that are hard blocked in the current sandbox level."""
        if self.sandbox_level < 3:
            # Only L3 has hard blocks
            return []

        hard_blocked = []
        for issue in self.issues:
            issue_type = issue.get("issue_type", "")
            rule_id = issue.get("rule_id", "")

            # Check if this issue type or rule is hard blocked
            if (issue_type in self.HARD_BLOCKED_ISSUE_TYPES_L3 or
                rule_id in self.HARD_BLOCKED_RULE_IDS_L3):
                hard_blocked.append(issue)

        return hard_blocked

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_safe": self.is_safe,
            "issues": self.issues,
            "scan_id": self.scan_id,
            "code_hash": self.code_hash,
            "high_severity_count": self.high_severity_count,
            "medium_severity_count": self.medium_severity_count,
            "low_severity_count": self.low_severity_count,
            "requires_confirmation": self.high_severity_count > 0 and not self.has_hard_blocked,
            "has_hard_blocked": self.has_hard_blocked,
            "hard_blocked_count": len(self.hard_blocked_issues),
            "sandbox_level": self.sandbox_level,
        }

    def format_report(self) -> str:
        """Format a human-readable security report."""
        if not self.issues:
            return "✅ Security scan passed. No issues found."

        lines = [
            f"📋 Security Scan Report (ID: {self.scan_id[:8]})",
            f"   Sandbox Level: L{self.sandbox_level}",
            f"   Found {len(self.issues)} item(s) for review:",
            "",
        ]

        severity_icons = {
            "Critical": "🔴",
            "High": "🟠",
            "Medium": "🟡",
            "Low": "🟢",
        }

        for idx, issue in enumerate(self.issues, 1):
            severity = issue.get("severity", "Medium")
            icon = severity_icons.get(severity, "⚪")

            # Mark hard blocked issues
            is_hard_blocked = issue in self.hard_blocked_issues
            block_marker = " 🚫 [HARD BLOCKED]" if is_hard_blocked else ""

            lines.append(f"  {icon} #{idx} [{severity}] {issue.get('issue_type', 'Unknown')}{block_marker}")
            lines.append(f"     ├─ Rule: {issue.get('rule_id', 'N/A')}")
            lines.append(f"     ├─ Line {issue.get('line_number', '?')}: {issue.get('description', '')}")
            lines.append(f"     └─ Code: {issue.get('code_snippet', '')[:60]}...")
            lines.append("")

        # Different messages based on whether there are hard blocked issues
        if self.has_hard_blocked:
            lines.append("🚫 HARD BLOCKED: This code contains operations that CANNOT be executed")
            lines.append(f"   in the current L{self.sandbox_level} sandbox environment.")
            lines.append("")
            lines.append("   The following operations are permanently blocked at runtime:")
            for issue in self.hard_blocked_issues:
                lines.append(f"   • {issue.get('issue_type', 'Unknown')}: {issue.get('description', '')}")
            lines.append("")
            lines.append("   ⚠️  Even with confirmation, this code will fail to execute.")
            lines.append("   Options:")
            lines.append("   1. Modify the code to remove blocked operations")
            lines.append("   2. Use a lower sandbox level (L1 or L2) if permitted")
        elif self.high_severity_count > 0:
            lines.append("⚠️  High severity issues found. Confirmation required to execute.")
            lines.append(f"   To proceed, call execute_code with confirmed=true and scan_id=\"{self.scan_id}\"")
        else:
            lines.append("ℹ️  Only low/medium severity issues found. Safe to execute.")

        return "\n".join(lines)


class SandboxExecutor:
    """Secure code execution sandbox using Rust skillbox."""
    
    # Cache scan results for confirmation flow (scan_id -> result)
    _scan_cache: Dict[str, SecurityScanResult] = {}
    # Cache expiry time in seconds
    SCAN_CACHE_TTL = 300  # 5 minutes
    
    def __init__(self):
        from ..sandbox.skillbox import find_binary
        
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
    
    def _create_temp_skill(self, language: str, code: str) -> Tuple[str, str]:
        """Create a temporary skill directory with the code file."""
        skill_dir = tempfile.mkdtemp(prefix="mcp_skill_")
        
        # Create scripts subdirectory (required by skillbox)
        scripts_dir = os.path.join(skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        ext = self.get_file_extension(language)
        entry_point = f"scripts/main.{ext}"
        
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
            f.write(code)
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
                result = subprocess.run(
                    [self.skillbox_path, "security-scan", "--json", code_file],
                    capture_output=True,
                    text=True,
                    timeout=30
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
            skill_dir, _ = self._create_temp_skill(language, code)
            
            try:
                env = os.environ.copy()
                env["SKILLBOX_AUTO_APPROVE"] = "true"
                
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


class MCPServer:
    """MCP server for SkillLite sandbox execution.
    
    This server provides tools for skills management and secure code execution:

    Skills Tools:
    1. **list_skills**: List all available skills in the skills directory.
    2. **get_skill_info**: Get detailed information about a specific skill.
    3. **run_skill**: Execute a skill with given input parameters.

    Code Execution Tools:
    4. **scan_code**: Scan code for security issues before execution.
       Returns a detailed report and a scan_id for confirmation.
    5. **execute_code**: Execute code in a sandbox. If high-severity
       security issues are found, requires explicit confirmation.

    Example workflow for skills:
        1. Call list_skills to see available skills
        2. Call get_skill_info to understand a skill's parameters
        3. Call run_skill with the required input

    Example workflow for code execution:
        1. Call scan_code to check for security issues
        2. Review the security report with the user
        3. If user approves, call execute_code with confirmed=true and scan_id
    """

    def __init__(self, skills_dir: Optional[str] = None):
        if not MCP_AVAILABLE:
            raise ImportError(
                "MCP library not available. Please install it with: "
                "pip install skilllite[mcp]"
            )
        self.server = Server("skilllite-mcp-server")
        self.executor = SandboxExecutor()

        # Initialize SkillManager for skills support
        self.skills_dir = skills_dir or os.environ.get("SKILLLITE_SKILLS_DIR", "./.skills")
        self.skill_manager = None
        self._init_skill_manager()

        self._setup_handlers()

    def _init_skill_manager(self):
        """Initialize the SkillManager if skills directory exists."""
        try:
            from ..core.manager import SkillManager
            skills_path = Path(self.skills_dir)
            if skills_path.exists():
                self.skill_manager = SkillManager(skills_dir=str(skills_path))
        except Exception as e:
            # Skills not available, continue without them
            pass
    
    def _setup_handlers(self):
        """Setup MCP server handlers."""

        @self.server.list_tools()
        async def list_tools() -> List[Tool]:
            tools = [
                # Skills management tools
                Tool(
                    name="list_skills",
                    description=(
                        "List all available skills in the skills directory. "
                        "Returns skill names, descriptions, and languages."
                    ),
                    inputSchema={
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                ),
                Tool(
                    name="get_skill_info",
                    description=(
                        "Get detailed information about a specific skill, "
                        "including its input schema, description, and usage."
                    ),
                    inputSchema={
                        "type": "object",
                        "properties": {
                            "skill_name": {
                                "type": "string",
                                "description": "Name of the skill to get info for"
                            }
                        },
                        "required": ["skill_name"]
                    }
                ),
                Tool(
                    name="run_skill",
                    description=(
                        "Execute a skill with the given input parameters. "
                        "Use list_skills to see available skills and "
                        "get_skill_info to understand required parameters. "
                        "IMPORTANT: If the skill has high-severity security issues, "
                        "you MUST show the security report to the user and ASK for their explicit confirmation "
                        "before setting confirmed=true. Do NOT auto-confirm without user approval."
                    ),
                    inputSchema={
                        "type": "object",
                        "properties": {
                            "skill_name": {
                                "type": "string",
                                "description": "Name of the skill to execute"
                            },
                            "input": {
                                "type": "object",
                                "description": "Input parameters for the skill"
                            },
                            "confirmed": {
                                "type": "boolean",
                                "description": (
                                    "Set to true ONLY after the user has explicitly approved execution. "
                                    "You must ask the user for confirmation first."
                                )
                            },
                            "scan_id": {
                                "type": "string",
                                "description": "Scan ID from security review (required when confirmed=true)"
                            }
                        },
                        "required": ["skill_name"]
                    }
                ),
                # Code execution tools
                Tool(
                    name="scan_code",
                    description=(
                        "Scan code for security issues before execution. "
                        "Returns a security report with any potential risks found. "
                        "Use this before execute_code to review security implications."
                    ),
                    inputSchema={
                        "type": "object",
                        "properties": {
                            "language": {
                                "type": "string",
                                "enum": ["python", "javascript", "bash"],
                                "description": "Programming language of the code"
                            },
                            "code": {
                                "type": "string",
                                "description": "Code to scan for security issues"
                            }
                        },
                        "required": ["language", "code"]
                    }
                ),
                Tool(
                    name="execute_code",
                    description=(
                        "Execute code in a secure sandbox environment. "
                        "IMPORTANT: If security issues are found, you MUST show the security report "
                        "to the user and ASK for their explicit confirmation before setting confirmed=true. "
                        "Do NOT auto-confirm without user approval."
                    ),
                    inputSchema={
                        "type": "object",
                        "properties": {
                            "language": {
                                "type": "string",
                                "enum": ["python", "javascript", "bash"],
                                "description": "Programming language to execute"
                            },
                            "code": {
                                "type": "string",
                                "description": "Code to execute"
                            },
                            "confirmed": {
                                "type": "boolean",
                                "description": (
                                    "Set to true ONLY after the user has explicitly approved execution. "
                                    "You must ask the user for confirmation first."
                                ),
                                "default": False
                            },
                            "scan_id": {
                                "type": "string",
                                "description": (
                                    "The scan_id from a previous scan_code call. "
                                    "Required when confirmed=true to verify the code hasn't changed."
                                )
                            },
                            "sandbox_level": {
                                "type": "integer",
                                "enum": [1, 2, 3],
                                "description": (
                                    "Sandbox security level: "
                                    "1=no sandbox, 2=sandbox only, 3=sandbox+security scan (default)"
                                ),
                                "default": 3
                            }
                        },
                        "required": ["language", "code"]
                    }
                )
            ]
            return tools
        
        @self.server.call_tool()
        async def call_tool(
            name: str,
            arguments: Dict[str, Any]
        ) -> "CallToolResult":
            # Skills management tools
            if name == "list_skills":
                return await self._handle_list_skills(arguments)
            elif name == "get_skill_info":
                return await self._handle_get_skill_info(arguments)
            elif name == "run_skill":
                return await self._handle_run_skill(arguments)
            # Code execution tools
            elif name == "scan_code":
                return await self._handle_scan_code(arguments)
            elif name == "execute_code":
                return await self._handle_execute_code(arguments)
            else:
                return CallToolResult(
                    isError=True,
                    content=[
                        TextContent(
                            type="text",
                            text=f"Unknown tool: {name}"
                        )
                    ]
                )
    
    async def _handle_scan_code(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle scan_code tool call."""
        language = arguments.get("language")
        code = arguments.get("code")
        
        if not language or not code:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required arguments: language and code"
                    )
                ]
            )
        
        scan_result = self.executor.scan_code(language, code)
        
        report = scan_result.format_report()
        
        result_json = json.dumps(scan_result.to_dict(), indent=2)
        
        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text=f"{report}\n\n---\nScan Details (JSON):\n{result_json}"
                )
            ]
        )
    
    async def _handle_execute_code(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle execute_code tool call."""
        language = arguments.get("language")
        code = arguments.get("code")
        confirmed = arguments.get("confirmed", False)
        scan_id = arguments.get("scan_id")
        # sandbox_level: None means use default from environment variable
        sandbox_level = arguments.get("sandbox_level")
        
        if not language or not code:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required arguments: language and code"
                    )
                ]
            )
        
        result = self.executor.execute(
            language=language,
            code=code,
            confirmed=confirmed,
            scan_id=scan_id,
            sandbox_level=sandbox_level,
        )
        
        # Handle hard blocked case - this is a definitive block, not a confirmation request
        if result.get("hard_blocked"):
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=result["stderr"]
                    )
                ]
            )

        if result.get("requires_confirmation"):
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=result["stderr"]
                    )
                ]
            )

        output_lines = []
        if result["stdout"]:
            output_lines.append(f"Output:\n{result['stdout']}")
        if result["stderr"]:
            output_lines.append(f"Errors:\n{result['stderr']}")

        output_text = "\n".join(output_lines) if output_lines else "Execution completed successfully (no output)"

        if result["success"]:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=output_text
                    )
                ]
            )
        else:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Execution failed:\n{output_text}"
                    )
                ]
            )

    async def _handle_list_skills(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle list_skills tool call."""
        if not self.skill_manager:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        skills = self.skill_manager.list_skills()
        if not skills:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills found in directory: {self.skills_dir}"
                    )
                ]
            )

        # Format skills list
        lines = ["Available Skills:", ""]
        for skill in skills:
            lines.append(f"• **{skill.name}**")
            if skill.description:
                lines.append(f"  {skill.description}")
            if skill.language:
                lines.append(f"  Language: {skill.language}")
            lines.append("")

        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text="\n".join(lines)
                )
            ]
        )

    async def _handle_get_skill_info(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """Handle get_skill_info tool call."""
        skill_name = arguments.get("skill_name")

        if not skill_name:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required argument: skill_name"
                    )
                ]
            )

        if not self.skill_manager:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        skill = self.skill_manager.get_skill(skill_name)
        if not skill:
            available = ", ".join(self.skill_manager.skill_names()) or "none"
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill not found: {skill_name}\nAvailable skills: {available}"
                    )
                ]
            )

        # Skill Usage Protocol - Phase 2 (Usage Phase):
        # Return the full SKILL.md content so LLM can infer correct
        # parameters from usage examples.
        full_content = skill.get_full_content()
        if not full_content:
            full_content = f"# {skill.name}\n\n{skill.description or 'No description available.'}"

        return CallToolResult(
            content=[
                TextContent(
                    type="text",
                    text=full_content
                )
            ]
        )

    async def _handle_run_skill(self, arguments: Dict[str, Any]) -> "CallToolResult":
        """
        Handle run_skill tool call using UnifiedExecutionService.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning
        3. Properly downgrades sandbox level after confirmation
        """
        skill_name = arguments.get("skill_name")
        input_data = arguments.get("input", {})
        confirmed = arguments.get("confirmed", False)
        scan_id = arguments.get("scan_id")

        if not skill_name:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text="Missing required argument: skill_name"
                    )
                ]
            )

        if not self.skill_manager:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"No skills available. Skills directory not found: {self.skills_dir}"
                    )
                ]
            )

        if not self.skill_manager.has_skill(skill_name):
            available = ", ".join(self.skill_manager.skill_names()) or "none"
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill not found: {skill_name}\nAvailable skills: {available}"
                    )
                ]
            )

        # Get skill info
        skill = self.skill_manager.get_skill(skill_name)
        if not skill:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Could not load skill: {skill_name}"
                    )
                ]
            )

        # Use UnifiedExecutionService
        from ..sandbox.execution_service import UnifiedExecutionService
        from ..sandbox.context import ExecutionContext

        service = UnifiedExecutionService.get_instance()

        # MCP uses async confirmation pattern (return report -> client calls back with confirmed=True)
        # Create a "callback" that captures the scan result for MCP's async flow
        scan_result_holder = {"result": None}

        def mcp_confirmation_callback(report: str, scan_id_from_scan: str) -> bool:
            # If client already confirmed, return True
            if confirmed:
                return True
            # Otherwise, store the result and return False to abort execution
            # MCP will then return the report to the client
            scan_result_holder["result"] = {"report": report, "scan_id": scan_id_from_scan}
            return False

        # Execute using unified service
        result = service.execute_skill(
            skill_info=skill,
            input_data=input_data,
            confirmation_callback=mcp_confirmation_callback if not confirmed else lambda r, s: True,
        )

        # Check if we need to return a security report (MCP async confirmation pattern)
        if scan_result_holder["result"] is not None:
            report_data = scan_result_holder["result"]
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=(
                            f"🔐 Security Review Required for skill '{skill_name}'\n\n"
                            f"{report_data['report']}\n\n"
                            f"⚠️ IMPORTANT: You MUST ask the user for confirmation before proceeding.\n"
                            f"Show this security report to the user and wait for their explicit approval.\n\n"
                            f"If the user approves, call run_skill again with:\n"
                            f"  - confirmed: true\n"
                            f"  - scan_id: \"{report_data['scan_id']}\"\n"
                        )
                    )
                ]
            )

        # Return execution result
        if result.success:
            return CallToolResult(
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill '{skill_name}' executed successfully.\n\nOutput:\n{result.output}"
                    )
                ]
            )
        else:
            return CallToolResult(
                isError=True,
                content=[
                    TextContent(
                        type="text",
                        text=f"Skill '{skill_name}' execution failed.\n\nError:\n{result.error}"
                    )
                ]
            )

    async def run(self):
        """Run the MCP server."""
        async with stdio_server() as (read_stream, write_stream):
            await self.server.run(read_stream, write_stream, self.server.create_initialization_options())


async def main():
    """Main entry point for the MCP sandbox server."""
    if not MCP_AVAILABLE:
        print("Error: MCP library not available")
        print("Please install it with: pip install skilllite[mcp]")
        return
    
    server = MCPServer()
    await server.run()


if __name__ == "__main__":
    asyncio.run(main())
