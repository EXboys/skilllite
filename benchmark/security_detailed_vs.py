#!/usr/bin/env python3
"""
SkillLite 细化安全性基准测试

这个脚本更精确地测试沙箱的安全行为，区分：
1. 操作本身是否被阻止（函数调用抛出异常）
2. 操作执行了但效果被限制（函数返回错误码或空结果）
3. 操作完全成功

测试维度：
- os.listdir('/') - 区分：抛出异常 vs 返回空列表 vs 返回完整列表
- os.system() - 区分：函数不可调用 vs 命令执行失败 vs 命令执行成功
- subprocess - 区分：模块不可导入 vs 执行失败 vs 执行成功
- 文件读写 - 区分：权限拒绝 vs 文件不存在 vs 成功
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
    """细化的安全测试结果"""
    FUNCTION_BLOCKED = "🔒 函数被阻止"      # 函数本身不可调用（抛出异常）
    EFFECT_LIMITED = "🛡️ 效果被限制"        # 函数可调用但效果被限制
    FULLY_ALLOWED = "❌ 完全允许"           # 操作完全成功
    ERROR = "⚙️ 测试错误"                   # 测试执行出错
    SKIPPED = "⏭️ 跳过"                     # 测试被跳过

@dataclass
class DetailedSecurityTest:
    """细化的安全测试用例"""
    name: str
    category: str
    description: str
    code: str
    timeout: int = 10

# ============================================================
# 细化的安全测试用例
# ============================================================

DETAILED_SECURITY_TESTS = [
    # ========== os.listdir 测试 ==========
    DetailedSecurityTest(
        name="listdir_root_detailed",
        category="文件系统",
        description="os.listdir('/') 细化测试",
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
        result["conclusion"] = "EFFECT_LIMITED"  # 函数可调用但返回空列表
    else:
        result["conclusion"] = "FULLY_ALLOWED"   # 完全成功
        
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
    
    # ========== os.system 测试 ==========
    DetailedSecurityTest(
        name="os_system_detailed",
        category="进程",
        description="os.system() 细化测试",
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

# 测试 1: 简单的 echo 命令（不依赖任何文件读取）
try:
    # 使用 echo 命令，它不需要读取任何系统文件
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
    
    # ========== os.system 执行 shell 命令测试 ==========
    DetailedSecurityTest(
        name="os_system_shell_detailed",
        category="进程",
        description="os.system() 执行 shell 命令细化测试",
        code='''
import json
import os
import tempfile

result = {
    "test": "os.system() with shell command",
    "tests": []
}

# 测试 1: 创建临时文件（测试 shell 是否真的在执行）
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

# 测试 2: 执行 /bin/sh -c
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

# 测试 3: 直接执行 Python（测试 process-exec）
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

# 总结
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
    
    # ========== subprocess 测试 ==========
    DetailedSecurityTest(
        name="subprocess_detailed",
        category="进程",
        description="subprocess 模块细化测试",
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

# 测试 1: 模块是否可导入
try:
    import subprocess
    result["module_importable"] = True
except ImportError as e:
    result["exception_type"] = "ImportError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    print(json.dumps(result, indent=2))
    sys.exit(0)

# 测试 2: subprocess.run 是否可调用
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

# 测试 3: subprocess.Popen 是否可调用
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

# 总结
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
    
    # ========== os.fork 测试 ==========
    DetailedSecurityTest(
        name="os_fork_detailed",
        category="进程",
        description="os.fork() 细化测试",
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
        # 子进程，立即退出
        os._exit(0)
    else:
        # 父进程
        result["function_callable"] = True
        result["child_pid"] = pid
        os.waitpid(pid, 0)  # 等待子进程结束
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
    
    # ========== 文件读取测试 ==========
    DetailedSecurityTest(
        name="file_read_detailed",
        category="文件系统",
        description="敏感文件读取细化测试",
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

# 总结
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
    
    # ========== 文件写入测试 ==========
    DetailedSecurityTest(
        name="file_write_detailed",
        category="文件系统",
        description="文件写入细化测试",
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
    "/etc/sandbox_test_write.txt",  # 应该被阻止
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
        # 清理
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

# 总结
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
    
    # ========== 网络测试 ==========
    DetailedSecurityTest(
        name="network_detailed",
        category="网络",
        description="网络访问细化测试",
        code='''
import json
import socket

result = {
    "test": "network access",
    "tests": []
}

# 测试 1: socket 模块是否可用
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

# 测试 2: 创建 socket
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

# 测试 3: DNS 查询
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

# 测试 4: TCP 连接
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

# 测试 5: 监听端口
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

# 总结
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
    
    # ========== ctypes 测试 ==========
    DetailedSecurityTest(
        name="ctypes_detailed",
        category="代码注入",
        description="ctypes 模块细化测试",
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

# 测试 1: 模块是否可导入
try:
    import ctypes
    result["module_importable"] = True
except ImportError as e:
    result["exception_type"] = "ImportError"
    result["exception_message"] = str(e)
    result["conclusion"] = "FUNCTION_BLOCKED"
    print(json.dumps(result, indent=2))
    sys.exit(0)

# 测试 2: CDLL 是否可访问
try:
    cdll = ctypes.CDLL
    result["cdll_accessible"] = True
except Exception as e:
    result["exception_message"] = str(e)

# 测试 3: 加载 libc
try:
    import ctypes.util
    libc_name = ctypes.util.find_library("c")
    if libc_name:
        libc = ctypes.CDLL(libc_name)
        result["libc_loadable"] = True
        
        # 测试 4: 调用 system()
        try:
            libc.system(b"echo ctypes_test > /dev/null 2>&1")
            result["system_callable"] = True
        except Exception as e:
            result["exception_message"] = f"system call failed: {e}"
except Exception as e:
    result["exception_message"] = str(e)

# 总结
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
    """检查命令是否可用"""
    return shutil.which(command) is not None


def check_claude_srt_available() -> bool:
    """检查 Claude SRT 是否可用"""
    if not check_command_available("srt"):
        return False
    try:
        result = subprocess.run(["srt", "--version"], capture_output=True, timeout=10)
        return result.returncode == 0
    except:
        return False


def check_skillbox_available(binary_path: str = None) -> tuple:
    """检查 skillbox 是否可用"""
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
    """Skillbox 细化安全测试"""
    
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
        """运行测试并返回详细结果"""
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
            
            # 尝试解析 JSON 输出
            try:
                # 找到 JSON 部分
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
    """Claude SRT 细化安全测试"""
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="claude_srt_detailed_")
    
    def run_test(self, test: DetailedSecurityTest) -> dict:
        """运行测试并返回详细结果"""
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
            
            # 尝试解析 JSON 输出
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
    """原生 Python 细化安全测试（作为基准）"""
    
    def run_test(self, test: DetailedSecurityTest) -> dict:
        """运行测试并返回详细结果"""
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
    """打印详细结果表格"""
    print("\n" + "=" * 100)
    print("细化安全测试结果")
    print("=" * 100)
    
    # 结论映射
    conclusion_display = {
        "FUNCTION_BLOCKED": "🔒 函数被阻止",
        "EFFECT_LIMITED": "🛡️ 效果被限制",
        "FULLY_ALLOWED": "❌ 完全允许",
        "ERROR": "⚙️ 错误",
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
                
                # 打印详细信息
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
    print("SkillLite 细化安全性基准测试")
    print("=" * 100)
    
    # 检查可用的测试平台
    platforms = []
    testers = {}
    
    # 原生 Python（作为基准）
    platforms.append("Native Python")
    testers["Native Python"] = DetailedNativePythonTest()
    
    # Claude SRT
    if check_claude_srt_available():
        platforms.append("Claude SRT")
        testers["Claude SRT"] = DetailedClaudeSRTTest()
        print("✅ Claude SRT 可用")
    else:
        print("⚠️ Claude SRT 不可用，跳过")
    
    # Skillbox
    skillbox_available, skillbox_path = check_skillbox_available()
    if skillbox_available:
        platforms.append("Skillbox")
        testers["Skillbox"] = DetailedSkillboxTest(skillbox_path)
        print(f"✅ Skillbox 可用: {skillbox_path}")
    else:
        print("⚠️ Skillbox 不可用，跳过")
    
    print(f"\n测试平台: {', '.join(platforms)}")
    print(f"测试用例数: {len(DETAILED_SECURITY_TESTS)}")
    
    # 运行测试
    results = {platform: {} for platform in platforms}
    
    for test in DETAILED_SECURITY_TESTS:
        print(f"\n运行测试: {test.description}...")
        
        for platform in platforms:
            tester = testers[platform]
            result = tester.run_test(test)
            results[platform][test.name] = result
            
            conclusion = result.get("conclusion", "ERROR")
            print(f"  {platform}: {conclusion}")
    
    # 打印详细结果
    print_detailed_results(results, platforms)
    
    # 清理
    for platform, tester in testers.items():
        if hasattr(tester, "cleanup"):
            tester.cleanup()
    
    # 打印对比总结
    print("\n" + "=" * 100)
    print("对比总结")
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
    
    # 打印表格
    header = f"| {'测试项'.ljust(35)} |"
    for platform in platforms:
        header += f" {platform.center(18)} |"
    print(header)
    print("|" + "-" * 37 + "|" + ("|" + "-" * 20) * len(platforms))
    
    conclusion_short = {
        "FUNCTION_BLOCKED": "🔒 阻止",
        "EFFECT_LIMITED": "🛡️ 限制",
        "FULLY_ALLOWED": "❌ 允许",
        "ERROR": "⚙️ 错误",
        "SKIPPED": "⏭️ 跳过",
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
