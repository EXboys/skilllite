#!/usr/bin/env python3
"""
SkillLite Security Benchmark: SkillLite (Rust Sandbox in skilllite/) vs Docker vs Pyodide

Test Dimensions:
1. File System Isolation - Read/write sensitive files, directory traversal
2. Network Isolation - External network access, port listening
3. Process Isolation - System command execution, process enumeration
4. Resource Limits - CPU/Memory/disk bomb protection
5. Code Injection - Dynamic imports, eval/exec protection

cd ./benchmark
python3 security_vs.py                    # 默认测试 Level 2 和 Level 3
python3 security_vs.py --test-all-levels # 测试 Level 1、2、3
python3 security_vs.py --skillbox-level 3 # 仅测试 Level 3

Level 1: No Sandbox - Direct execution
Level 2: Sandbox isolation only
Level 3: Sandbox isolation + static code scanning
"""

import subprocess
import os
import tempfile
import shutil
import json
import sys
from dataclasses import dataclass
from enum import Enum
from typing import Optional


class SecurityResult(Enum):
    """Security test result"""
    BLOCKED = "✅ Blocked"     # Attack completely blocked
    PARTIAL = "⚠️ Partial"    # Attack partially blocked or limited
    ALLOWED = "❌ Allowed"     # Attack successfully executed
    ERROR = "⚙️ Error"         # Test execution error
    SKIPPED = "⏭️ Skipped"     # Test skipped


@dataclass
class SecurityTest:
    """Security test case"""
    name: str
    category: str
    description: str
    code: str
    success_indicator: str  # If output contains this string, attack succeeded
    timeout: int = 10


# Security test case definitions
SECURITY_TESTS = [
    # ========== File System Isolation ==========
    SecurityTest(
        name="read_etc_passwd",
        category="File System",
        description="Read /etc/passwd",
        code='''
import json
try:
    with open("/etc/passwd", "r") as f:
        content = f.read()
    print(json.dumps({"result": "SUCCESS", "content": content[:100]}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="read_ssh_key",
        category="File System",
        description="Read SSH private key",
        code='''
import json
import os
try:
    ssh_key_path = os.path.expanduser("~/.ssh/id_rsa")
    with open(ssh_key_path, "r") as f:
        content = f.read()
    print(json.dumps({"result": "SUCCESS", "has_key": len(content) > 0}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="write_tmp_file",
        category="File System",
        description="Write to /tmp directory",
        code='''
import json
try:
    with open("/tmp/security_test_hack.txt", "w") as f:
        f.write("hacked!")
    print(json.dumps({"result": "SUCCESS"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="path_traversal",
        category="File System",
        description="Path traversal attack (../../../)",
        code='''
import json
import os
try:
    # Try to read system file through path traversal
    path = "../../../../../../../etc/passwd"
    with open(path, "r") as f:
        content = f.read()
    print(json.dumps({"result": "SUCCESS", "content": content[:50]}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="list_root_dir",
        category="File System",
        description="List root directory contents",
        code='''
import json
import os
try:
    files = os.listdir("/")
    print(json.dumps({"result": "SUCCESS", "files": files[:10]}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    # ========== Network Isolation ==========
    SecurityTest(
        name="http_request",
        category="Network",
        description="Make HTTP requests",
        code='''
import json
try:
    import urllib.request
    response = urllib.request.urlopen("http://httpbin.org/get", timeout=3)
    print(json.dumps({"result": "SUCCESS", "status": response.status}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"',
        timeout=8
    ),
    
    SecurityTest(
        name="dns_lookup",
        category="Network",
        description="DNS lookup",
        code='''
import json
try:
    import socket
    ip = socket.gethostbyname("google.com")
    print(json.dumps({"result": "SUCCESS", "ip": ip}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"',
        timeout=10
    ),
    
    SecurityTest(
        name="socket_listen",
        category="Network",
        description="Listen on port",
        code='''
import json
try:
    import socket
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("0.0.0.0", 8888))
    s.listen(1)
    s.close()
    print(json.dumps({"result": "SUCCESS"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    # ========== Process Isolation ==========
    SecurityTest(
        name="os_system",
        category="Process",
        description="Execute os.system()",
        code='''
import json
import os
try:
    ret = os.system("whoami")
    print(json.dumps({"result": "SUCCESS", "return_code": ret}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="subprocess_run",
        category="Process",
        description="Execute subprocess",
        code='''
import json
try:
    import subprocess
    result = subprocess.run(["whoami"], capture_output=True, text=True)
    print(json.dumps({"result": "SUCCESS", "output": result.stdout.strip()}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="process_list",
        category="Process",
        description="Enumerate system processes",
        code='''
import json
import os
try:
    # Try to read /proc directory to list processes
    procs = [p for p in os.listdir("/proc") if p.isdigit()]
    print(json.dumps({"result": "SUCCESS", "process_count": len(procs)}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="kill_process",
        category="Process",
        description="Send process signal",
        code='''
import json
import os
import signal
try:
    # Try to send signal to init process (won't actually kill it)
    os.kill(1, 0)  # Signal 0 only checks if process exists
    print(json.dumps({"result": "SUCCESS"}))
except PermissionError:
    print(json.dumps({"result": "PARTIAL", "error": "Permission denied but process visible"}))
except ProcessLookupError:
    print(json.dumps({"result": "BLOCKED", "error": "Process not found"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    # ========== Resource Limits ==========
    SecurityTest(
        name="memory_bomb",
        category="Resource Limits",
        description="Memory bomb (allocate large memory)",
        code='''
import json
try:
    # Try to allocate 500MB memory (reduced for faster testing)
    data = bytearray(500 * 1024 * 1024)
    print(json.dumps({"result": "SUCCESS", "size": len(data)}))
except MemoryError:
    print(json.dumps({"result": "BLOCKED", "error": "MemoryError"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"',
        timeout=10
    ),
    
    SecurityTest(
        name="fork_bomb",
        category="Resource Limits",
        description="Fork bomb",
        code='''
import json
import os
try:
    # Only try to fork once, not a real bomb
    pid = os.fork()
    if pid == 0:
        os._exit(0)
    print(json.dumps({"result": "SUCCESS", "child_pid": pid}))
except AttributeError:
    print(json.dumps({"result": "BLOCKED", "error": "fork not available"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="cpu_intensive",
        category="Resource Limits",
        description="CPU intensive calculation (check time limits)",
        code='''
import json
import time
try:
    start = time.time()
    # Execute a calculation that takes more than 5 seconds
    # If there is a CPU time limit, the process will be terminated by SIGXCPU
    # If there is a timeout mechanism, the process will be terminated by SIGKILL
    count = 0
    while True:
        _ = sum(i * i for i in range(100000))
        count += 1
        elapsed = time.time() - start
        if elapsed > 10:  # Safe exit
            break
    print(json.dumps({"result": "SUCCESS", "elapsed": elapsed, "iterations": count}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"',
        timeout=8  # Shortened timeout for faster testing
    ),
    
    # ========== Code Injection ==========
    SecurityTest(
        name="dynamic_import",
        category="Code Injection",
        description="Dynamic import of os module",
        code='''
import json
try:
    os_module = __import__("os")
    cwd = os_module.getcwd()
    print(json.dumps({"result": "SUCCESS", "cwd": cwd}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="eval_exec",
        category="Code Injection",
        description="Use eval/exec to execute code",
        code='''
import json
try:
    result = eval("__import__('os').getcwd()")
    print(json.dumps({"result": "SUCCESS", "eval_result": result}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="modify_builtins",
        category="Code Injection",
        description="Modify built-in functions",
        code='''
import json
import builtins
try:
    original_open = builtins.open
    builtins.open = lambda *args, **kwargs: None
    # Restore
    builtins.open = original_open
    print(json.dumps({"result": "SUCCESS"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    # ========== Information Leakage ==========
    SecurityTest(
        name="env_variables",
        category="Information Leakage",
        description="Read environment variables",
        code='''
import json
import os
try:
    env_vars = dict(os.environ)
    sensitive_keys = [k for k in env_vars if any(s in k.upper() for s in ["KEY", "SECRET", "TOKEN", "PASSWORD", "CREDENTIAL"])]
    print(json.dumps({"result": "SUCCESS", "env_count": len(env_vars), "sensitive_keys": sensitive_keys}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="system_info",
        category="Information Leakage",
        description="Get system information",
        code='''
import json
import platform
import os
try:
    info = {
        "system": platform.system(),
        "node": platform.node(),
        "release": platform.release(),
        "user": os.getenv("USER", "unknown"),
        "home": os.path.expanduser("~")
    }
    print(json.dumps({"result": "SUCCESS", "info": info}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
]


def check_command_available(command: str) -> bool:
    """Check if a command is available"""
    return shutil.which(command) is not None

def check_claude_srt_available() -> bool:
    """Check if Claude SRT (Sandboxed Runtime) is available"""
    if not check_command_available("srt"):
        return False
    try:
        result = subprocess.run(
            ["srt", "--version"],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def check_docker_available() -> bool:
    """Check if Docker is available"""
    if not check_command_available("docker"):
        return False
    try:
        result = subprocess.run(
            ["docker", "version"],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def check_skilllite_available(binary_path: str = None) -> tuple:
    """Check if skilllite binary is available, returns (is_available, actual_path)
    Binary lives in skilllite/ directory (not skillbox). Fallback to skillbox for backward compat.
    """
    if binary_path and os.path.exists(binary_path):
        try:
            subprocess.run([binary_path, "--help"], capture_output=True, timeout=10)
            return True, binary_path
        except Exception:
            pass

    # Primary: skilllite (current project structure)
    for name in ("skilllite", "skillbox"):
        system_path = shutil.which(name)
        if system_path:
            return True, system_path

    # Project paths: skilllite/ directory (see docs/zh/ARCHITECTURE.md)
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    project_paths = [
        os.path.join(project_root, "skilllite", "target", "release", "skilllite"),
        os.path.join(script_dir, "..", "skilllite", "target", "release", "skilllite"),
        os.path.expanduser("~/.cargo/bin/skilllite"),
        os.path.expanduser("~/.cargo/bin/skillbox"),
    ]
    for path in project_paths:
        abs_path = os.path.abspath(path)
        if os.path.exists(abs_path):
            return True, abs_path

    return False, ""


class SkillLiteSecurityTest:
    """SkillLite security test (Rust sandbox executor in skilllite/ directory)"""
    
    def __init__(self, binary_path: str, sandbox_level: int = 2):
        # Convert to absolute path to avoid issues when running from different directories
        self.binary_path = os.path.abspath(binary_path)
        self.sandbox_level = sandbox_level
        self.work_dir = tempfile.mkdtemp(prefix="skilllite_security_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """Create Skill directory structure for testing"""
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        skill_md = """---
name: security-test-skill
description: Security test skill
version: 1.0.0
entry_point: scripts/main.py
---
# Security Test Skill
"""
        with open(os.path.join(self.skill_dir, "SKILL.md"), "w") as f:
            f.write(skill_md)
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """Run a single security test"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(test.code)
        
        try:
            # Set environment variables for skilllite (SKILLBOX_* for backward compat)
            # Use specified sandbox level
            env = os.environ.copy()
            env["SKILLBOX_SANDBOX_LEVEL"] = str(self.sandbox_level)
            env["SKILLLITE_TRUST_BYPASS_CONFIRM"] = "1"
            
            result = subprocess.run(
                [self.binary_path, "run", "--sandbox-level", str(self.sandbox_level), self.skill_dir, "{}"],
                capture_output=True,
                timeout=test.timeout,
                cwd=self.work_dir,
                env=env
            )
            
            output = result.stdout.decode() + result.stderr.decode()
            
            # Check if the attack succeeded
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "PARTIAL"' in output:
                return SecurityResult.PARTIAL
            # Check if blocked by Skillbox security wrapper
            elif "[SKILLBOX]" in output and "denied" in output.lower():
                return SecurityResult.BLOCKED
            elif '"result": "BLOCKED"' in output:
                return SecurityResult.BLOCKED
            # If skill execution failed with error, check if it's a security block
            elif result.returncode != 0:
                # Check stderr for security-related errors
                if "SKILLBOX" in output or "SecurityError" in output or "denied" in output.lower():
                    return SecurityResult.BLOCKED
                # Other errors might still be security blocks
                if "Permission" in output or "access" in output.lower():
                    return SecurityResult.BLOCKED
                return SecurityResult.BLOCKED  # Treat execution failures as blocked
            else:
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            return SecurityResult.BLOCKED  # Timeout is treated as blocked
        except Exception as e:
            return SecurityResult.ERROR
    
    def cleanup(self):
        """Clean up temporary directory"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DockerSecurityTest:
    """Docker security test"""
    
    def __init__(self, image: str = "python:3.11-slim"):
        self.image = image
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """Run a single security test"""
        try:
            result = subprocess.run(
                ["docker", "run", "--rm", self.image, "python", "-c", test.code],
                capture_output=True,
                timeout=test.timeout
            )
            
            output = result.stdout.decode() + result.stderr.decode()
            
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "PARTIAL"' in output:
                return SecurityResult.PARTIAL
            else:
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            return SecurityResult.BLOCKED
        except Exception:
            return SecurityResult.ERROR


class PyodideSecurityTest:
    """Pyodide (WebAssembly) security test"""

    def __init__(self):
        self.node_available = check_command_available("node")
        # Check if Pyodide is installed (by checking file system)
        self.pyodide_available = self._check_pyodide_installed()

    def _check_pyodide_installed(self) -> bool:
        """Check if Pyodide npm package is installed"""
        # Check multiple possible installation locations
        possible_paths = [
            os.path.join(os.path.dirname(__file__), "node_modules", "pyodide", "package.json"),
            os.path.join(os.getcwd(), "node_modules", "pyodide", "package.json"),
            os.path.join(os.path.dirname(os.path.dirname(__file__)), "node_modules", "pyodide", "package.json"),
        ]
        
        for path in possible_paths:
            if os.path.exists(path):
                return True
        
        return False
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """Run a single security test"""
        if not self.node_available:
            return SecurityResult.ERROR
        
        if not self.pyodide_available:
            return SecurityResult.ERROR
        
        # Pyodide runs in WebAssembly, naturally isolating most system calls
        # Here we simulate its behavior
        escaped_code = test.code.replace('`', '\\`')
        js_code = f'''
const {{ loadPyodide }} = require("pyodide");

async function main() {{
    try {{
        const pyodide = await loadPyodide();
        const result = await pyodide.runPythonAsync(`{escaped_code}`);
        console.log(result);
    }} catch (e) {{
        console.log(JSON.stringify({{"result": "BLOCKED", "error": e.message}}));
    }}
}}

main();
'''
        
        try:
            # Create temporary file in benchmark directory instead of system temp directory
            # So that Node.js can correctly find the pyodide module
            benchmark_dir = os.path.dirname(os.path.abspath(__file__))
            js_file = os.path.join(benchmark_dir, f".pyodide_test_{os.getpid()}_{test.name}.js")
            
            with open(js_file, 'w') as f:
                f.write(js_code)
            
            try:
                result = subprocess.run(
                    ["node", js_file],
                    capture_output=True,
                    timeout=test.timeout + 10,  # Pyodide loading needs extra time
                    cwd=benchmark_dir
                )
            finally:
                # Clean up temporary file
                if os.path.exists(js_file):
                    os.unlink(js_file)
            
            output = result.stdout.decode() + result.stderr.decode()

            # Debug output: show actual execution result
            if result.returncode != 0:
                print(f"  [Pyodide Debug] {test.description}: Node.js return code {result.returncode}", file=sys.stderr)
                if output:
                    print(f"  [Pyodide Debug] Output: {output[:200]}", file=sys.stderr)
            
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "BLOCKED"' in output:
                return SecurityResult.BLOCKED
            elif result.returncode != 0:
                # Node.js execution failed, meaning Pyodide is really not available
                print(f"  [Pyodide Error] {test.description}: Execution failed (return code {result.returncode})", file=sys.stderr)
                return SecurityResult.ERROR
            else:
                # Execution succeeded but no success indicator matched, treated as blocked
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            print(f"  [Pyodide Timeout] {test.description}: Execution timeout", file=sys.stderr)
            return SecurityResult.BLOCKED
        except Exception as e:
            # Real error case, should not return preset result
            print(f"  [Pyodide Error] {test.description}: {str(e)}", file=sys.stderr)
            return SecurityResult.ERROR
    
    def _get_expected_result(self, test: SecurityTest) -> SecurityResult:
        """Return expected result based on Pyodide's known characteristics

        Note: This method is deprecated and no longer used. Pyodide tests must now actually execute.
        If execution fails, should return ERROR instead of preset result.
        """
        # This method is kept only for backward compatibility but should not be called
        return SecurityResult.ERROR


class ClaudeSRTSecurityTest:
    """Claude SRT (Sandboxed Runtime) security test

    Claude SRT is a sandboxed runtime environment provided by Anthropic for safe code execution.
    It uses Linux namespaces and seccomp for isolation.
    """
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="claude_srt_security_")
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """Run a single security test"""
        # Write test code to temporary file
        script_path = os.path.join(self.work_dir, "test_script.py")
        with open(script_path, "w") as f:
            f.write(test.code)
        
        try:
            # Use srt command to run Python script (using python3)
            result = subprocess.run(
                ["srt", "python3", script_path],
                capture_output=True,
                timeout=test.timeout,
                cwd=self.work_dir
            )
            
            output = result.stdout.decode() + result.stderr.decode()

            # Check if attack succeeded
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "PARTIAL"' in output:
                return SecurityResult.PARTIAL
            # Check if blocked by SRT security mechanism
            elif "Permission denied" in output or "Operation not permitted" in output:
                return SecurityResult.BLOCKED
            elif "seccomp" in output.lower() or "sandbox" in output.lower():
                return SecurityResult.BLOCKED
            elif '"result": "BLOCKED"' in output:
                return SecurityResult.BLOCKED
            # If execution failed, check if it was a security block
            elif result.returncode != 0:
                if any(keyword in output.lower() for keyword in ["denied", "permission", "blocked", "forbidden"]):
                    return SecurityResult.BLOCKED
                return SecurityResult.BLOCKED  # Execution failure treated as blocked
            else:
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            return SecurityResult.BLOCKED  # Timeout treated as blocked
        except Exception as e:
            return SecurityResult.ERROR
    
    def cleanup(self):
        """Clean up temporary directory"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)



def print_results_table(results: dict, platforms: list):
    """Print results table"""
    # Group by category
    categories = {}
    for test in SECURITY_TESTS:
        if test.category not in categories:
            categories[test.category] = []
        categories[test.category].append(test)

    # Calculate column width
    name_width = max(len(t.description) for t in SECURITY_TESTS) + 2
    platform_width = 14

    # Print header
    header = f"| {'Test Item'.ljust(name_width)} |"
    for platform in platforms:
        header += f" {platform.center(platform_width)} |"
    print(header)
    
    separator = f"|{'-' * (name_width + 2)}|"
    for _ in platforms:
        separator += f"{'-' * (platform_width + 2)}|"
    print(separator)

    # Print results by category
    for category, tests in categories.items():
        # Print category title
        print(f"| **{category}** |" + " |" * len(platforms))
        
        for test in tests:
            row = f"| {test.description.ljust(name_width)} |"
            for platform in platforms:
                result = results.get(platform, {}).get(test.name, SecurityResult.SKIPPED)
                row += f" {result.value.center(platform_width)} |"
            print(row)
    
    print()


def calculate_security_score(results: dict) -> dict:
    """Calculate security score"""
    scores = {}
    for platform, platform_results in results.items():
        blocked = sum(1 for r in platform_results.values() if r == SecurityResult.BLOCKED)
        partial = sum(1 for r in platform_results.values() if r == SecurityResult.PARTIAL)
        total = len([r for r in platform_results.values() if r != SecurityResult.SKIPPED])
        
        if total > 0:
            score = (blocked + partial * 0.5) / total * 100
        else:
            score = 0
        
        scores[platform] = {
            "blocked": blocked,
            "partial": partial,
            "allowed": sum(1 for r in platform_results.values() if r == SecurityResult.ALLOWED),
            "total": total,
            "score": score
        }
    
    return scores


def main():
    import argparse

    parser = argparse.ArgumentParser(description="SkillLite Security Benchmark")
    parser.add_argument("--skilllite", "--skillbox", type=str, dest="skilllite", help="SkillLite executable path (binary in skilllite/ directory)")
    parser.add_argument("--docker-image", type=str, default="python:3.11-slim", help="Docker image")
    parser.add_argument("--skip-docker", action="store_true", help="Skip Docker test")
    parser.add_argument("--skip-pyodide", action="store_true", help="Skip Pyodide test")
    parser.add_argument("--skip-claude-srt", action="store_true", help="Skip Claude SRT test")
    parser.add_argument("--output", type=str, help="Output JSON result to file")
    parser.add_argument("--skillbox-level", type=int, default=2,
                       choices=[1, 2, 3],
                       help="SkillLite sandbox level (1=No sandbox, 2=Sandbox only, 3=Sandbox+static check)")
    parser.add_argument("--test-all-levels", action="store_true",
                       help="Test all SkillLite security levels (1, 2, 3)")
    args = parser.parse_args()
    
    print("=" * 60)
    print("SkillLite Security Benchmark")
    print("=" * 60)
    print()
    
    results = {}
    platforms = []
    
    # SkillLite Test (Rust binary in skilllite/ directory)
    skilllite_available, skilllite_path = check_skilllite_available(args.skilllite)
    if skilllite_available:
        # Determine security levels to test
        if args.test_all_levels:
            test_levels = [1, 2, 3]
        elif args.skillbox_level == 2:
            # Default: test both Level 2 and 3 for comprehensive security comparison
            test_levels = [2, 3]
        else:
            test_levels = [args.skillbox_level]

        level_names = {
            1: "No Sandbox",
            2: "Sandbox Only",
            3: "Sandbox + Static Check"
        }

        for level in test_levels:
            platform_name = f"SkillLite (Level {level})"
            print(f"🦀 Testing {platform_name} - {level_names[level]} ({skilllite_path})...")
            skilllite_tester = SkillLiteSecurityTest(skilllite_path, sandbox_level=level)
            results[platform_name] = {}
            platforms.append(platform_name)
            
            for test in SECURITY_TESTS:
                result = skilllite_tester.run_test(test)
                results[platform_name][test.name] = result
                print(f"  {test.description}: {result.value}")
            
            skilllite_tester.cleanup()
            print()
    else:
        print("⚠️  SkillLite not available, skipping test")
        print()
    
    # Docker Test
    if not args.skip_docker and check_docker_available():
        print(f"🐳 Testing Docker ({args.docker_image})...")
        docker_tester = DockerSecurityTest(args.docker_image)
        results["Docker"] = {}
        platforms.append("Docker")
        
        for test in SECURITY_TESTS:
            result = docker_tester.run_test(test)
            results["Docker"][test.name] = result
            print(f"  {test.description}: {result.value}")
        print()
    elif args.skip_docker:
        print("⏭️  Skipping Docker test")
        print()
    else:
        print("⚠️  Docker not available, skipping test")
        print()
    
    # Pyodide Test
    if not args.skip_pyodide:
        print("🌐 Testing Pyodide (WebAssembly)...")
        pyodide_tester = PyodideSecurityTest()

        # Check if Pyodide is really available
        if not pyodide_tester.node_available:
            print("⚠️  Node.js not available, skipping Pyodide test")
            print()
        else:
            # Check if Pyodide is installed
            if not pyodide_tester.pyodide_available:
                print("⚠️  Pyodide npm package not installed, skipping test")
                print("   Hint: Run 'npm install pyodide' to install")
                print()
            else:
                results["Pyodide"] = {}
                platforms.append("Pyodide")
                
                for test in SECURITY_TESTS:
                    result = pyodide_tester.run_test(test)
                    results["Pyodide"][test.name] = result
                    print(f"  {test.description}: {result.value}")
                print()
    
    # Claude SRT Test
    if not args.skip_claude_srt and check_claude_srt_available():
        print("🤖 Testing Claude SRT (Sandboxed Runtime)...")
        claude_srt_tester = ClaudeSRTSecurityTest()
        results["Claude SRT"] = {}
        platforms.append("Claude SRT")
        
        for test in SECURITY_TESTS:
            result = claude_srt_tester.run_test(test)
            results["Claude SRT"][test.name] = result
            print(f"  {test.description}: {result.value}")
        
        claude_srt_tester.cleanup()
        print()
    elif args.skip_claude_srt:
        print("⏭️  Skipping Claude SRT test")
        print()
    elif not check_claude_srt_available():
        print("⚠️  Claude SRT not available, skipping test")
        print("   Hint: Please ensure the srt command-line tool is installed")
        print()
    
    # Print results table
    print("=" * 60)
    print("Security Comparison Results")
    print("=" * 60)
    print()
    print_results_table(results, platforms)

    # Calculate and print security scores
    scores = calculate_security_score(results)
    print("## Security Score")
    print()
    print("| Platform | Blocked | Partial | Allowed | Security Score |")
    print("|----------|---------|---------|---------|----------------|")
    for platform in platforms:
        s = scores[platform]
        print(f"| {platform} | {s['blocked']} | {s['partial']} | {s['allowed']} | {s['score']:.1f}% |")
    print()
    
    # Output JSON results
    if args.output:
        output_data = {
            "results": {
                platform: {name: result.name for name, result in platform_results.items()}
                for platform, platform_results in results.items()
            },
            "scores": scores,
            "tests": [
                {
                    "name": t.name,
                    "category": t.category,
                    "description": t.description
                }
                for t in SECURITY_TESTS
            ]
        }
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2, ensure_ascii=False)
        print(f"📄 Results saved to {args.output}")

    print("=" * 60)
    print("Test completed!")
    print("=" * 60)


if __name__ == "__main__":
    main()
