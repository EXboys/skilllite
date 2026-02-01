#!/usr/bin/env python3
"""
SkillLite Detailed Security Benchmark

This script more precisely tests sandbox security behavior, distinguishing:
1. Whether the operation itself is blocked (function call throws exception)
2. Operation executed but effect is limited (function returns error code or empty result)
3. Operation completely succeeded

Test dimensions:
- os.listdir('/') - Distinguish: throws exception vs returns empty list vs returns full list
- os.system() - Distinguish: function not callable vs command execution failed vs command execution succeeded
- subprocess - Distinguish: module not importable vs execution failed vs execution succeeded
- File read/write - Distinguish: permission denied vs file not found vs success
"""

import subprocess
import os
import tempfile
import shutil
import json
import sys
from dataclasses import dataclass
from enum import Enum
from typing import Optional, Dict, List

class DetailedResult(Enum):
    """Detailed security test result"""
    FUNCTION_BLOCKED = "🔒 Function Blocked"   # Function itself not callable (throws exception)
    EFFECT_LIMITED = "🛡️ Effect Limited"       # Function callable but effect is limited
    FULLY_ALLOWED = "❌ Fully Allowed"          # Operation completely succeeded
    ERROR = "⚙️ Test Error"                     # Test execution error
    SKIPPED = "⏭️ Skipped"                      # Test was skipped

@dataclass
class DetailedSecurityTest:
    """Detailed security test case"""
    name: str
    category: str
    description: str
    code: str
    timeout: int = 10

# ============================================================
# Detailed security test cases
# ============================================================

DETAILED_SECURITY_TESTS = [
    # ========== os.listdir Test ==========
    DetailedSecurityTest(
        name="listdir_root_detailed",
        category="File System",
        description="os.listdir('/') Detailed Test",
        code='''
import json
import os

result = {
    "test": "os.listdir('/')",
    "function_callable": False,
    "exception_type": None,
    "exception_message": None,
    "return_value": None,
    "file_count": 0,
    "conclusion": None
}

try:
    files = os.listdir("/")
    result["function_callable"] = True
    result["return_value"] = files[:20] if files else []
    result["file_count"] = len(files)
    
    if len(files) == 0:
        result["conclusion"] = "EFFECT_LIMITED"  # Function callable but returns empty list
    else:
        result["conclusion"] = "FULLY_ALLOWED"   # Fully succeeded
        
except PermissionError as e:
    result["exception_type"] = "PermissionError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except OSError as e:
    result["exception_type"] = "OSError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except Exception as e:
    result["exception_type"] = type(e).__name__
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== os.system Test ==========
    DetailedSecurityTest(
        name="os_system_detailed",
        category="Process",
        description="os.system() Detailed Test",
        code='''
import json
import os
import sys

result = {
    "test": "os.system()",
    "function_exists": hasattr(os, "system"),
    "function_callable": False,
    "exception_type": None,
    "exception_message": None,
    "return_code": None,
    "command_output_captured": False,
    "conclusion": None
}

if not result["function_exists"]:
    result["conclusion"] = "FUNCTION_BLOCKED"
    print(json.dumps(result, indent=2))
    sys.exit(0)

# Test 1: Simple echo command (does not depend on any file reading)
try:
    # Use echo command, which doesn't need to read any system files
    ret = os.system("echo 'sandbox_test_marker_12345' > /dev/null 2>&1")
    result["function_callable"] = True
    result["return_code"] = ret
    
    if ret == 0:
        result["conclusion"] = "FULLY_ALLOWED"
    else:
        result["conclusion"] = "EFFECT_LIMITED"
        
except AttributeError as e:
    result["exception_type"] = "AttributeError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except PermissionError as e:
    result["exception_type"] = "PermissionError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except OSError as e:
    result["exception_type"] = "OSError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except Exception as e:
    result["exception_type"] = type(e).__name__
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== os.system Execute shell command Test ==========
    DetailedSecurityTest(
        name="os_system_shell_detailed",
        category="Process",
        description="os.system() Execute shell command Detailed Test",
        code='''
import json
import os
import tempfile

result = {
    "test": "os.system() with shell command",
    "tests": []
}

# Test 1: Create temporary file (test if shell is actually executing)
test1 = {
    "command": "touch /tmp/sandbox_test_file",
    "success": False,
    "return_code": None,
    "file_created": False
}
try:
    ret = os.system("touch /tmp/sandbox_test_file_12345 2>/dev/null")
    test1["return_code"] = ret
    test1["success"] = (ret == 0)
    test1["file_created"] = os.path.exists("/tmp/sandbox_test_file_12345")
    if test1["file_created"]:
        os.remove("/tmp/sandbox_test_file_12345")
except Exception as e:
    test1["error"] = str(e)
result["tests"].append(test1)

# Test 2: Execute /bin/sh -c
test2 = {
    "command": "/bin/sh -c 'echo test'",
    "success": False,
    "return_code": None
}
try:
    ret = os.system("/bin/sh -c 'echo test' > /dev/null 2>&1")
    test2["return_code"] = ret
    test2["success"] = (ret == 0)
except Exception as e:
    test2["error"] = str(e)
result["tests"].append(test2)

# Test 3: Execute Python directly (test process-exec)
test3 = {
    "command": "python3 -c 'print(1)'",
    "success": False,
    "return_code": None
}
try:
    ret = os.system("python3 -c 'print(1)' > /dev/null 2>&1")
    test3["return_code"] = ret
    test3["success"] = (ret == 0)
except Exception as e:
    test3["error"] = str(e)
result["tests"].append(test3)

# Summary
all_blocked = all(not t.get("success", False) for t in result["tests"])
all_allowed = all(t.get("success", False) for t in result["tests"])

if all_blocked:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif all_allowed:
    result["conclusion"] = "FULLY_ALLOWED"
else:
    result["conclusion"] = "EFFECT_LIMITED"

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== subprocess Test ==========
    DetailedSecurityTest(
        name="subprocess_detailed",
        category="Process",
        description="subprocess Module Detailed Test",
        code='''
import json
import sys

result = {
    "test": "subprocess module",
    "module_importable": False,
    "popen_callable": False,
    "run_callable": False,
    "exception_type": None,
    "exception_message": None,
    "tests": [],
    "conclusion": None
}

# Test 1: Check if module is importable
try:
    import subprocess
    result["module_importable"] = True
except ImportError as e:
    result["exception_type"] = "ImportError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    print(json.dumps(result, indent=2))
    sys.exit(0)

# Test 2: Check if subprocess.run is callable
test_run = {
    "function": "subprocess.run",
    "callable": False,
    "success": False,
    "return_code": None,
    "stdout": None,
    "error": None
}
try:
    proc = subprocess.run(
        ["echo", "test"],
        capture_output=True,
        text=True,
        timeout=5
    )
    test_run["callable"] = True
    test_run["return_code"] = proc.returncode
    test_run["stdout"] = proc.stdout.strip()
    test_run["success"] = (proc.returncode == 0 and "test" in proc.stdout)
    result["run_callable"] = True
except PermissionError as e:
    test_run["error"] = f"PermissionError: {e}"
except OSError as e:
    test_run["error"] = f"OSError: {e}"
except Exception as e:
    test_run["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_run)

# Test 3: Check if subprocess.Popen is callable
test_popen = {
    "function": "subprocess.Popen",
    "callable": False,
    "success": False,
    "return_code": None,
    "stdout": None,
    "error": None
}
try:
    proc = subprocess.Popen(
        ["echo", "popen_test"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    stdout, stderr = proc.communicate(timeout=5)
    test_popen["callable"] = True
    test_popen["return_code"] = proc.returncode
    test_popen["stdout"] = stdout.strip()
    test_popen["success"] = (proc.returncode == 0 and "popen_test" in stdout)
    result["popen_callable"] = True
except PermissionError as e:
    test_popen["error"] = f"PermissionError: {e}"
except OSError as e:
    test_popen["error"] = f"OSError: {e}"
except Exception as e:
    test_popen["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_popen)

# Summary
if not result["module_importable"]:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif not result["run_callable"] and not result["popen_callable"]:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif all(t.get("success", False) for t in result["tests"]):
    result["conclusion"] = "FULLY_ALLOWED"
else:
    result["conclusion"] = "EFFECT_LIMITED"

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== os.fork Test ==========
    DetailedSecurityTest(
        name="os_fork_detailed",
        category="Process",
        description="os.fork() Detailed Test",
        code='''
import json
import os
import sys

result = {
    "test": "os.fork()",
    "function_exists": hasattr(os, "fork"),
    "function_callable": False,
    "exception_type": None,
    "exception_message": None,
    "child_pid": None,
    "conclusion": None
}

if not result["function_exists"]:
    result["conclusion"] = "FUNCTION_BLOCKED"
    result["exception_message"] = "os.fork not available on this platform"
    print(json.dumps(result, indent=2))
    sys.exit(0)

try:
    pid = os.fork()
    if pid == 0:
        # Child process, exit immediately
        os._exit(0)
    else:
        # Parent process
        result["function_callable"] = True
        result["child_pid"] = pid
        os.waitpid(pid, 0)  # Wait for child process to finish
        result["conclusion"] = "FULLY_ALLOWED"
        
except PermissionError as e:
    result["exception_type"] = "PermissionError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except OSError as e:
    result["exception_type"] = "OSError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    
except Exception as e:
    result["exception_type"] = type(e).__name__
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== File Read Test ==========
    DetailedSecurityTest(
        name="file_read_detailed",
        category="File System",
        description="Sensitive File Read Detailed Test",
        code='''
import json
import os

result = {
    "test": "sensitive file read",
    "tests": []
}

sensitive_files = [
    "/etc/passwd",
    "/etc/shadow",
    os.path.expanduser("~/.ssh/id_rsa"),
    os.path.expanduser("~/.bash_history"),
    "/private/etc/passwd",  # macOS
]

for filepath in sensitive_files:
    test = {
        "file": filepath,
        "exists": os.path.exists(filepath),
        "readable": False,
        "content_length": 0,
        "exception_type": None,
        "exception_message": None
    }
    
    try:
        with open(filepath, "r") as f:
            content = f.read()
            test["readable"] = True
            test["content_length"] = len(content)
    except FileNotFoundError as e:
        test["exception_type"] = "FileNotFoundError"
        test["exception_message"] = str(e)
    except PermissionError as e:
        test["exception_type"] = "PermissionError"
        test["exception_message"] = str(e)
    except OSError as e:
        test["exception_type"] = "OSError"
        test["exception_message"] = str(e)
    except Exception as e:
        test["exception_type"] = type(e).__name__
        test["exception_message"] = str(e)
    
    result["tests"].append(test)

# Summary
readable_count = sum(1 for t in result["tests"] if t["readable"])
total_existing = sum(1 for t in result["tests"] if t["exists"])

if readable_count == 0:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif readable_count < total_existing:
    result["conclusion"] = "EFFECT_LIMITED"
else:
    result["conclusion"] = "FULLY_ALLOWED"

result["summary"] = {
    "total_files": len(sensitive_files),
    "existing_files": total_existing,
    "readable_files": readable_count
}

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== File Write Test ==========
    DetailedSecurityTest(
        name="file_write_detailed",
        category="File System",
        description="File Write Detailed Test",
        code='''
import json
import os
import tempfile

result = {
    "test": "file write",
    "tests": []
}

write_targets = [
    "/tmp/sandbox_test_write.txt",
    "/private/tmp/sandbox_test_write.txt",  # macOS
    "/etc/sandbox_test_write.txt",  # Should be blocked
    os.path.expanduser("~/.sandbox_test_write.txt"),
]

for filepath in write_targets:
    test = {
        "file": filepath,
        "writable": False,
        "file_created": False,
        "exception_type": None,
        "exception_message": None
    }
    
    try:
        with open(filepath, "w") as f:
            f.write("sandbox_test_content")
        test["writable"] = True
        test["file_created"] = os.path.exists(filepath)
        # Cleanup
        if test["file_created"]:
            os.remove(filepath)
    except PermissionError as e:
        test["exception_type"] = "PermissionError"
        test["exception_message"] = str(e)
    except OSError as e:
        test["exception_type"] = "OSError"
        test["exception_message"] = str(e)
    except Exception as e:
        test["exception_type"] = type(e).__name__
        test["exception_message"] = str(e)
    
    result["tests"].append(test)

# Summary
writable_count = sum(1 for t in result["tests"] if t["writable"])

if writable_count == 0:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif writable_count < len(write_targets):
    result["conclusion"] = "EFFECT_LIMITED"
else:
    result["conclusion"] = "FULLY_ALLOWED"

result["summary"] = {
    "total_targets": len(write_targets),
    "writable_targets": writable_count
}

print(json.dumps(result, indent=2))
'''
    ),
    
    # ========== Network Test ==========
    DetailedSecurityTest(
        name="network_detailed",
        category="Network",
        description="Network Access Detailed Test",
        code='''
import json
import socket

result = {
    "test": "network access",
    "tests": []
}

# Test 1: Check if socket module is available
test_socket = {
    "test": "socket module import",
    "success": False,
    "error": None
}
try:
    import socket
    test_socket["success"] = True
except ImportError as e:
    test_socket["error"] = str(e)
result["tests"].append(test_socket)

# Test 2: Create socket
test_create = {
    "test": "socket creation",
    "success": False,
    "error": None
}
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    test_create["success"] = True
    s.close()
except Exception as e:
    test_create["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_create)

# Test 3: DNS lookup
test_dns = {
    "test": "DNS lookup",
    "success": False,
    "ip": None,
    "error": None
}
try:
    ip = socket.gethostbyname("google.com")
    test_dns["success"] = True
    test_dns["ip"] = ip
except Exception as e:
    test_dns["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_dns)

# Test 4: TCP connection
test_connect = {
    "test": "TCP connect to google.com:80",
    "success": False,
    "error": None
}
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(3)
    s.connect(("google.com", 80))
    test_connect["success"] = True
    s.close()
except Exception as e:
    test_connect["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_connect)

# Test 5: Listen on port
test_listen = {
    "test": "listen on port 18888",
    "success": False,
    "error": None
}
try:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(("127.0.0.1", 18888))
    s.listen(1)
    test_listen["success"] = True
    s.close()
except Exception as e:
    test_listen["error"] = f"{type(e).__name__}: {e}"
result["tests"].append(test_listen)

# Summary
success_count = sum(1 for t in result["tests"] if t.get("success", False))

if success_count == 0:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif success_count < len(result["tests"]):
    result["conclusion"] = "EFFECT_LIMITED"
else:
    result["conclusion"] = "FULLY_ALLOWED"

result["summary"] = {
    "total_tests": len(result["tests"]),
    "successful_tests": success_count
}

print(json.dumps(result, indent=2))
''',
        timeout=15
    ),
    
    # ========== ctypes Test ==========
    DetailedSecurityTest(
        name="ctypes_detailed",
        category="Code Injection",
        description="ctypes Module Detailed Test",
        code='''
import json
import sys

result = {
    "test": "ctypes module",
    "module_importable": False,
    "cdll_accessible": False,
    "libc_loadable": False,
    "system_callable": False,
    "exception_type": None,
    "exception_message": None,
    "conclusion": None
}

# Test 1: Check if module is importable
try:
    import ctypes
    result["module_importable"] = True
except ImportError as e:
    result["exception_type"] = "ImportError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    print(json.dumps(result, indent=2))
    sys.exit(0)

# Test 2: Check if CDLL is accessible
try:
    cdll = ctypes.CDLL
    result["cdll_accessible"] = True
except Exception as e:
    result["exception_message"] = str(e)

# Test 3: Load libc
try:
    import ctypes.util
    libc_name = ctypes.util.find_library("c")
    if libc_name:
        libc = ctypes.CDLL(libc_name)
        result["libc_loadable"] = True

        # Test 4: Call system()
        try:
            libc.system(b"echo ctypes_test > /dev/null 2>&1")
            result["system_callable"] = True
        except Exception as e:
            result["exception_message"] = f"system call failed: {e}"
except Exception as e:
    result["exception_message"] = str(e)

# Summary
if not result["module_importable"]:
    result["conclusion"] = "FUNCTION_BLOCKED"
elif not result["libc_loadable"]:
    result["conclusion"] = "EFFECT_LIMITED"
elif not result["system_callable"]:
    result["conclusion"] = "EFFECT_LIMITED"
else:
    result["conclusion"] = "FULLY_ALLOWED"

print(json.dumps(result, indent=2))
'''
    ),
]


def check_command_available(command: str) -> bool:
    """Check if command is available"""
    return shutil.which(command) is not None


def check_claude_srt_available() -> bool:
    """Check if Claude SRT is available"""
    if not check_command_available("srt"):
        return False
    try:
        result = subprocess.run(["srt", "--version"], capture_output=True, timeout=10)
        return result.returncode == 0
    except:
        return False


def check_skillbox_available(binary_path: str = None) -> tuple:
    """Check if skillbox is available"""
    if binary_path and os.path.exists(binary_path):
        return True, binary_path
    
    system_path = shutil.which("skillbox")
    if system_path:
        return True, system_path
    
    project_paths = [
        "./skillbox/target/release/skillbox",
        "../skillbox/target/release/skillbox",
        os.path.expanduser("~/.cargo/bin/skillbox"),
    ]
    for path in project_paths:
        if os.path.exists(path):
            return True, path
    
    return False, ""


class DetailedSkillboxTest:
    """Skillbox detailed security test"""
    
    def __init__(self, binary_path: str):
        self.binary_path = os.path.abspath(binary_path)
        self.work_dir = tempfile.mkdtemp(prefix="skillbox_detailed_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        skill_md = """---
name: detailed-security-test
description: Detailed security test skill
version: 1.0.0
entry_point: scripts/main.py
---
# Detailed Security Test Skill
"""
        with open(os.path.join(self.skill_dir, "SKILL.md"), "w") as f:
            f.write(skill_md)
    
    def run_test(self, test: DetailedSecurityTest) -> dict:
        """Run test and return detailed result"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(test.code)

        try:
            result = subprocess.run(
                [self.binary_path, "run", self.skill_dir, "{}"],
                capture_output=True,
                timeout=test.timeout,
                cwd=self.work_dir
            )

            output = result.stdout.decode() + result.stderr.decode()

            # Try to parse JSON output
            try:
                # Find JSON section
                json_start = output.find('{')
                json_end = output.rfind('}') + 1
                if json_start >= 0 and json_end > json_start:
                    json_str = output[json_start:json_end]
                    return json.loads(json_str)
            except json.JSONDecodeError:
                pass
            
            return {
                "error": "Failed to parse output",
                "raw_output": output[:1000],
                "conclusion": "ERROR"
            }
            
        except subprocess.TimeoutExpired:
            return {"error": "Timeout", "conclusion": "FUNCTION_BLOCKED"}
        except Exception as e:
            return {"error": str(e), "conclusion": "ERROR"}
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DetailedClaudeSRTTest:
    """Claude SRT detailed security test"""

    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="claude_srt_detailed_")

    def run_test(self, test: DetailedSecurityTest) -> dict:
        """Run test and return detailed result"""
        script_path = os.path.join(self.work_dir, "test_script.py")
        with open(script_path, "w") as f:
            f.write(test.code)

        try:
            result = subprocess.run(
                ["srt", "python3", script_path],
                capture_output=True,
                timeout=test.timeout,
                cwd=self.work_dir
            )

            output = result.stdout.decode() + result.stderr.decode()

            # Try to parse JSON output
            try:
                json_start = output.find('{')
                json_end = output.rfind('}') + 1
                if json_start >= 0 and json_end > json_start:
                    json_str = output[json_start:json_end]
                    return json.loads(json_str)
            except json.JSONDecodeError:
                pass
            
            return {
                "error": "Failed to parse output",
                "raw_output": output[:1000],
                "conclusion": "ERROR"
            }
            
        except subprocess.TimeoutExpired:
            return {"error": "Timeout", "conclusion": "FUNCTION_BLOCKED"}
        except Exception as e:
            return {"error": str(e), "conclusion": "ERROR"}
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DetailedNativePythonTest:
    """Native Python detailed security test (as baseline)"""

    def run_test(self, test: DetailedSecurityTest) -> dict:
        """Run test and return detailed result"""
        try:
            result = subprocess.run(
                [sys.executable, "-c", test.code],
                capture_output=True,
                timeout=test.timeout
            )
            
            output = result.stdout.decode() + result.stderr.decode()
            
            try:
                json_start = output.find('{')
                json_end = output.rfind('}') + 1
                if json_start >= 0 and json_end > json_start:
                    json_str = output[json_start:json_end]
                    return json.loads(json_str)
            except json.JSONDecodeError:
                pass
            
            return {
                "error": "Failed to parse output",
                "raw_output": output[:1000],
                "conclusion": "ERROR"
            }
            
        except subprocess.TimeoutExpired:
            return {"error": "Timeout", "conclusion": "FUNCTION_BLOCKED"}
        except Exception as e:
            return {"error": str(e), "conclusion": "ERROR"}


def print_detailed_results(results: Dict[str, Dict[str, dict]], platforms: List[str]):
    """Print detailed results table"""
    print("\n" + "=" * 100)
    print("Detailed Security Test Results")
    print("=" * 100)

    # Conclusion mapping
    conclusion_display = {
        "FUNCTION_BLOCKED": "🔒 Function Blocked",
        "EFFECT_LIMITED": "🛡️ Effect Limited",
        "FULLY_ALLOWED": "❌ Fully Allowed",
        "ERROR": "⚙️ Error",
    }
    
    for test in DETAILED_SECURITY_TESTS:
        print(f"\n### {test.description} ({test.name})")
        print("-" * 80)
        
        for platform in platforms:
            if platform in results and test.name in results[platform]:
                result = results[platform][test.name]
                conclusion = result.get("conclusion", "ERROR")
                display = conclusion_display.get(conclusion, conclusion)
                
                print(f"\n**{platform}**: {display}")

                # Print detailed information
                if "tests" in result:
                    for t in result["tests"]:
                        if isinstance(t, dict):
                            test_name = t.get("test", t.get("command", t.get("file", "unknown")))
                            success = t.get("success", t.get("readable", t.get("writable", False)))
                            error = t.get("error", t.get("exception_message", ""))
                            status = "✅" if success else "❌"
                            print(f"  {status} {test_name}")
                            if error:
                                print(f"      Error: {error[:80]}")
                
                if "summary" in result:
                    print(f"  Summary: {result['summary']}")
                
                if "exception_type" in result and result["exception_type"]:
                    print(f"  Exception: {result['exception_type']}: {result.get('exception_message', '')[:80]}")


def main():
    print("=" * 100)
    print("SkillLite Detailed Security Benchmark")
    print("=" * 100)

    # Check available test platforms
    platforms = []
    testers = {}

    # Native Python (as baseline)
    platforms.append("Native Python")
    testers["Native Python"] = DetailedNativePythonTest()

    # Claude SRT
    if check_claude_srt_available():
        platforms.append("Claude SRT")
        testers["Claude SRT"] = DetailedClaudeSRTTest()
        print("✅ Claude SRT available")
    else:
        print("⚠️ Claude SRT not available, skipping")

    # Skillbox
    skillbox_available, skillbox_path = check_skillbox_available()
    if skillbox_available:
        platforms.append("Skillbox")
        testers["Skillbox"] = DetailedSkillboxTest(skillbox_path)
        print(f"✅ Skillbox available: {skillbox_path}")
    else:
        print("⚠️ Skillbox not available, skipping")

    print(f"\nTest platforms: {', '.join(platforms)}")
    print(f"Test cases: {len(DETAILED_SECURITY_TESTS)}")

    # Run tests
    results = {platform: {} for platform in platforms}

    for test in DETAILED_SECURITY_TESTS:
        print(f"\nRunning test: {test.description}...")
        
        for platform in platforms:
            tester = testers[platform]
            result = tester.run_test(test)
            results[platform][test.name] = result
            
            conclusion = result.get("conclusion", "ERROR")
            print(f"  {platform}: {conclusion}")
    
    # Print detailed results
    print_detailed_results(results, platforms)

    # Cleanup
    for platform, tester in testers.items():
        if hasattr(tester, "cleanup"):
            tester.cleanup()

    # Print comparison summary
    print("\n" + "=" * 100)
    print("Comparison Summary")
    print("=" * 100)

    summary_table = []
    for test in DETAILED_SECURITY_TESTS:
        row = {"test": test.description}
        for platform in platforms:
            if platform in results and test.name in results[platform]:
                row[platform] = results[platform][test.name].get("conclusion", "ERROR")
            else:
                row[platform] = "SKIPPED"
        summary_table.append(row)

    # Print table
    header = f"| {'Test Item'.ljust(35)} |"
    for platform in platforms:
        header += f" {platform.center(18)} |"
    print(header)
    print("|" + "-" * 37 + "|" + ("|" + "-" * 20) * len(platforms))
    
    conclusion_short = {
        "FUNCTION_BLOCKED": "🔒 Blocked",
        "EFFECT_LIMITED": "🛡️ Limited",
        "FULLY_ALLOWED": "❌ Allowed",
        "ERROR": "⚙️ Error",
        "SKIPPED": "⏭️ Skipped",
    }
    
    for row in summary_table:
        line = f"| {row['test'].ljust(35)} |"
        for platform in platforms:
            val = row.get(platform, "SKIPPED")
            display = conclusion_short.get(val, val)
            line += f" {display.center(18)} |"
        print(line)


if __name__ == "__main__":
    main()
