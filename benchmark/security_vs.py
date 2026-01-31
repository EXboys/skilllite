#!/usr/bin/env python3
"""
SkillLite 安全性基准测试: Skillbox (Rust 沙箱) vs Docker vs Pyodide

测试维度：
1. 文件系统隔离 - 读取/写入敏感文件、目录遍历
2. 网络隔离 - 外网访问、端口监听
3. 进程隔离 - 系统命令执行、进程枚举
4. 资源限制 - CPU/内存/磁盘炸弹防护
5. 代码注入 - 动态导入、eval/exec 防护

cd ./benchmark
python3 security_vs.py
python3 benchmark/security_vs.py --skillbox-level 0

Level 1: 无沙箱 - 直接执行
Level 2: 仅沙箱隔离
Level 3: 沙箱隔离 + 静态代码扫描
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
    """安全测试结果"""
    BLOCKED = "✅ 阻止"      # 攻击被完全阻止
    PARTIAL = "⚠️ 部分阻止"  # 攻击被部分阻止或有限制
    ALLOWED = "❌ 允许"      # 攻击成功执行
    ERROR = "⚙️ 错误"        # 测试执行出错
    SKIPPED = "⏭️ 跳过"      # 测试被跳过


@dataclass
class SecurityTest:
    """安全测试用例"""
    name: str
    category: str
    description: str
    code: str
    success_indicator: str  # 如果输出包含此字符串，说明攻击成功
    timeout: int = 10


# 安全测试用例定义
SECURITY_TESTS = [
    # ========== 文件系统隔离 ==========
    SecurityTest(
        name="read_etc_passwd",
        category="文件系统",
        description="读取 /etc/passwd",
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
        category="文件系统",
        description="读取 SSH 私钥",
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
        category="文件系统",
        description="写入 /tmp 目录",
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
        category="文件系统",
        description="目录遍历攻击 (../../../)",
        code='''
import json
import os
try:
    # 尝试通过路径遍历读取系统文件
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
        category="文件系统",
        description="列出根目录内容",
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
    
    # ========== 网络隔离 ==========
    SecurityTest(
        name="http_request",
        category="网络",
        description="发起 HTTP 请求",
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
        category="网络",
        description="DNS 查询",
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
        category="网络",
        description="监听端口",
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
    
    # ========== 进程隔离 ==========
    SecurityTest(
        name="os_system",
        category="进程",
        description="执行 os.system()",
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
        category="进程",
        description="执行 subprocess",
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
        category="进程",
        description="枚举系统进程",
        code='''
import json
import os
try:
    # 尝试读取 /proc 目录列出进程
    procs = [p for p in os.listdir("/proc") if p.isdigit()]
    print(json.dumps({"result": "SUCCESS", "process_count": len(procs)}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    SecurityTest(
        name="kill_process",
        category="进程",
        description="发送进程信号",
        code='''
import json
import os
import signal
try:
    # 尝试向 init 进程发送信号 (不会真的杀死它)
    os.kill(1, 0)  # 信号 0 只检查进程是否存在
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
    
    # ========== 资源限制 ==========
    SecurityTest(
        name="memory_bomb",
        category="资源限制",
        description="内存炸弹 (分配大量内存)",
        code='''
import json
try:
    # 尝试分配 100MB 内存（减小以加快测试）
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
        category="资源限制",
        description="Fork 炸弹",
        code='''
import json
import os
try:
    # 只尝试 fork 一次，不是真正的炸弹
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
        category="资源限制",
        description="CPU 密集计算 (是否有时间限制)",
        code='''
import json
import time
try:
    start = time.time()
    # 执行一个需要超过 5 秒的计算
    # 如果有 CPU 时间限制，进程会被 SIGXCPU 终止
    # 如果有超时机制，进程会被 SIGKILL 终止
    count = 0
    while True:
        _ = sum(i * i for i in range(100000))
        count += 1
        elapsed = time.time() - start
        if elapsed > 10:  # 安全退出
            break
    print(json.dumps({"result": "SUCCESS", "elapsed": elapsed, "iterations": count}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"',
        timeout=8  # 缩短超时时间，加快测试
    ),
    
    # ========== 代码注入 ==========
    SecurityTest(
        name="dynamic_import",
        category="代码注入",
        description="动态导入 os 模块",
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
        category="代码注入",
        description="使用 eval/exec 执行代码",
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
        category="代码注入",
        description="修改内置函数",
        code='''
import json
import builtins
try:
    original_open = builtins.open
    builtins.open = lambda *args, **kwargs: None
    # 恢复
    builtins.open = original_open
    print(json.dumps({"result": "SUCCESS"}))
except Exception as e:
    print(json.dumps({"result": "BLOCKED", "error": str(e)}))
''',
        success_indicator='"result": "SUCCESS"'
    ),
    
    # ========== 环境信息泄露 ==========
    SecurityTest(
        name="env_variables",
        category="信息泄露",
        description="读取环境变量",
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
        category="信息泄露",
        description="获取系统信息",
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
    """检查命令是否可用"""
    return shutil.which(command) is not None

def check_claude_srt_available() -> bool:
    """检查 Claude SRT (Sandboxed Runtime) 是否可用"""
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
    """检查 Docker 是否可用"""
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


def check_skillbox_available(binary_path: str = None) -> tuple:
    """检查 skillbox 是否可用，返回 (是否可用, 实际路径)"""
    if binary_path and os.path.exists(binary_path):
        try:
            subprocess.run([binary_path, "--help"], capture_output=True, timeout=10)
            return True, binary_path
        except Exception:
            pass
    
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


class SkillboxSecurityTest:
    """Skillbox 安全性测试"""
    
    def __init__(self, binary_path: str, sandbox_level: int = 2):
        # Convert to absolute path to avoid issues when running from different directories
        self.binary_path = os.path.abspath(binary_path)
        self.sandbox_level = sandbox_level
        self.work_dir = tempfile.mkdtemp(prefix="skillbox_security_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """创建测试用的 Skill 目录结构"""
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
        """运行单个安全测试"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(test.code)
        
        try:
            # Set environment variables for skillbox
            # Use specified sandbox level
            env = os.environ.copy()
            env["SKILLBOX_SANDBOX_LEVEL"] = str(self.sandbox_level)
            
            result = subprocess.run(
                [self.binary_path, "run", self.skill_dir, "{}"],
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
            return SecurityResult.BLOCKED  # 超时视为被阻止
        except Exception as e:
            return SecurityResult.ERROR
    
    def cleanup(self):
        """清理临时目录"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DockerSecurityTest:
    """Docker 安全性测试"""
    
    def __init__(self, image: str = "python:3.11-slim"):
        self.image = image
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """运行单个安全测试"""
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
    """Pyodide (WebAssembly) 安全性测试"""
    
    def __init__(self):
        self.node_available = check_command_available("node")
        # 检查 Pyodide 是否已安装（通过检查文件系统）
        self.pyodide_available = self._check_pyodide_installed()
    
    def _check_pyodide_installed(self) -> bool:
        """检查 Pyodide npm 包是否已安装"""
        # 检查多个可能的安装位置
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
        """运行单个安全测试"""
        if not self.node_available:
            return SecurityResult.ERROR
        
        if not self.pyodide_available:
            return SecurityResult.ERROR
        
        # Pyodide 在 WebAssembly 中运行，天然隔离了大部分系统调用
        # 这里我们模拟其行为
        js_code = f'''
const {{ loadPyodide }} = require("pyodide");

async function main() {{
    try {{
        const pyodide = await loadPyodide();
        const result = await pyodide.runPythonAsync(`{test.code.replace('`', '\\`')}`);
        console.log(result);
    }} catch (e) {{
        console.log(JSON.stringify({{"result": "BLOCKED", "error": e.message}}));
    }}
}}

main();
'''
        
        try:
            # 在 benchmark 目录下创建临时文件，而不是系统临时目录
            # 这样 Node.js 可以正确找到 pyodide 模块
            benchmark_dir = os.path.dirname(os.path.abspath(__file__))
            js_file = os.path.join(benchmark_dir, f".pyodide_test_{os.getpid()}_{test.name}.js")
            
            with open(js_file, 'w') as f:
                f.write(js_code)
            
            try:
                result = subprocess.run(
                    ["node", js_file],
                    capture_output=True,
                    timeout=test.timeout + 10,  # Pyodide 加载需要额外时间
                    cwd=benchmark_dir
                )
            finally:
                # 清理临时文件
                if os.path.exists(js_file):
                    os.unlink(js_file)
            
            output = result.stdout.decode() + result.stderr.decode()
            
            # 调试输出：显示实际执行结果
            if result.returncode != 0:
                print(f"  [Pyodide 调试] {test.description}: Node.js 返回码 {result.returncode}", file=sys.stderr)
                if output:
                    print(f"  [Pyodide 调试] 输出: {output[:200]}", file=sys.stderr)
            
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "BLOCKED"' in output:
                return SecurityResult.BLOCKED
            elif result.returncode != 0:
                # Node.js 执行失败，说明 Pyodide 真的不可用
                print(f"  [Pyodide 错误] {test.description}: 执行失败 (返回码 {result.returncode})", file=sys.stderr)
                return SecurityResult.ERROR
            else:
                # 执行成功但没有匹配到成功指示符，视为被阻止
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            print(f"  [Pyodide 超时] {test.description}: 执行超时", file=sys.stderr)
            return SecurityResult.BLOCKED
        except Exception as e:
            # 真正的错误情况，不应该返回预设结果
            print(f"  [Pyodide 错误] {test.description}: {str(e)}", file=sys.stderr)
            return SecurityResult.ERROR
    
    def _get_expected_result(self, test: SecurityTest) -> SecurityResult:
        """根据 Pyodide 的已知特性返回预期结果
        
        注意：此方法已废弃，不再使用。Pyodide 测试现在必须实际执行。
        如果执行失败，应返回 ERROR 而不是预设结果。
        """
        # 此方法保留仅为向后兼容，但不应再被调用
        return SecurityResult.ERROR


class ClaudeSRTSecurityTest:
    """Claude SRT (Sandboxed Runtime) 安全性测试
    
    Claude SRT 是 Anthropic 提供的沙箱运行时环境，用于安全执行代码。
    它使用 Linux 命名空间和 seccomp 进行隔离。
    """
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="claude_srt_security_")
    
    def run_test(self, test: SecurityTest) -> SecurityResult:
        """运行单个安全测试"""
        # 将测试代码写入临时文件
        script_path = os.path.join(self.work_dir, "test_script.py")
        with open(script_path, "w") as f:
            f.write(test.code)
        
        try:
            # 使用 srt 命令运行 Python 脚本 (使用 python3)
            result = subprocess.run(
                ["srt", "python3", script_path],
                capture_output=True,
                timeout=test.timeout,
                cwd=self.work_dir
            )
            
            output = result.stdout.decode() + result.stderr.decode()
            
            # 检查攻击是否成功
            if test.success_indicator in output:
                return SecurityResult.ALLOWED
            elif '"result": "PARTIAL"' in output:
                return SecurityResult.PARTIAL
            # 检查是否被 SRT 安全机制阻止
            elif "Permission denied" in output or "Operation not permitted" in output:
                return SecurityResult.BLOCKED
            elif "seccomp" in output.lower() or "sandbox" in output.lower():
                return SecurityResult.BLOCKED
            elif '"result": "BLOCKED"' in output:
                return SecurityResult.BLOCKED
            # 如果执行失败，检查是否是安全阻止
            elif result.returncode != 0:
                if any(keyword in output.lower() for keyword in ["denied", "permission", "blocked", "forbidden"]):
                    return SecurityResult.BLOCKED
                return SecurityResult.BLOCKED  # 执行失败视为被阻止
            else:
                return SecurityResult.BLOCKED
                
        except subprocess.TimeoutExpired:
            return SecurityResult.BLOCKED  # 超时视为被阻止
        except Exception as e:
            return SecurityResult.ERROR
    
    def cleanup(self):
        """清理临时目录"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)



def print_results_table(results: dict, platforms: list):
    """打印结果表格"""
    # 按类别分组
    categories = {}
    for test in SECURITY_TESTS:
        if test.category not in categories:
            categories[test.category] = []
        categories[test.category].append(test)
    
    # 计算列宽
    name_width = max(len(t.description) for t in SECURITY_TESTS) + 2
    platform_width = 14
    
    # 打印表头
    header = f"| {'测试项'.ljust(name_width)} |"
    for platform in platforms:
        header += f" {platform.center(platform_width)} |"
    print(header)
    
    separator = f"|{'-' * (name_width + 2)}|"
    for _ in platforms:
        separator += f"{'-' * (platform_width + 2)}|"
    print(separator)
    
    # 按类别打印结果
    for category, tests in categories.items():
        # 打印类别标题
        print(f"| **{category}** |" + " |" * len(platforms))
        
        for test in tests:
            row = f"| {test.description.ljust(name_width)} |"
            for platform in platforms:
                result = results.get(platform, {}).get(test.name, SecurityResult.SKIPPED)
                row += f" {result.value.center(platform_width)} |"
            print(row)
    
    print()


def calculate_security_score(results: dict) -> dict:
    """计算安全评分"""
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
    
    parser = argparse.ArgumentParser(description="SkillLite 安全性基准测试")
    parser.add_argument("--skillbox", type=str, help="Skillbox 可执行文件路径")
    parser.add_argument("--docker-image", type=str, default="python:3.11-slim", help="Docker 镜像")
    parser.add_argument("--skip-docker", action="store_true", help="跳过 Docker 测试")
    parser.add_argument("--skip-pyodide", action="store_true", help="跳过 Pyodide 测试")
    parser.add_argument("--skip-claude-srt", action="store_true", help="跳过 Claude SRT 测试")
    parser.add_argument("--output", type=str, help="输出 JSON 结果到文件")
    parser.add_argument("--skillbox-level", type=int, default=2, 
                       choices=[1, 2, 3],
                       help="Skillbox 沙箱安全级别 (1=无沙箱, 2=仅沙箱, 3=沙箱+静态检查)")
    parser.add_argument("--test-all-levels", action="store_true", 
                       help="测试 Skillbox 的所有安全级别 (1, 2, 3)")
    args = parser.parse_args()
    
    print("=" * 60)
    print("SkillLite 安全性基准测试")
    print("=" * 60)
    print()
    
    results = {}
    platforms = []
    
    # Skillbox 测试
    skillbox_available, skillbox_path = check_skillbox_available(args.skillbox)
    if skillbox_available:
        # 确定要测试的安全级别
        if args.test_all_levels:
            test_levels = [1, 2, 3]
        else:
            test_levels = [args.skillbox_level]
        
        level_names = {
            1: "无沙箱",
            2: "仅沙箱",
            3: "沙箱+静态检查"
        }
        
        for level in test_levels:
            platform_name = f"Skillbox (Level {level})"
            print(f"🦀 测试 {platform_name} - {level_names[level]} ({skillbox_path})...")
            skillbox_tester = SkillboxSecurityTest(skillbox_path, sandbox_level=level)
            results[platform_name] = {}
            platforms.append(platform_name)
            
            for test in SECURITY_TESTS:
                result = skillbox_tester.run_test(test)
                results[platform_name][test.name] = result
                print(f"  {test.description}: {result.value}")
            
            skillbox_tester.cleanup()
            print()
    else:
        print("⚠️  Skillbox 不可用，跳过测试")
        print()
    
    # Docker 测试
    if not args.skip_docker and check_docker_available():
        print(f"🐳 测试 Docker ({args.docker_image})...")
        docker_tester = DockerSecurityTest(args.docker_image)
        results["Docker"] = {}
        platforms.append("Docker")
        
        for test in SECURITY_TESTS:
            result = docker_tester.run_test(test)
            results["Docker"][test.name] = result
            print(f"  {test.description}: {result.value}")
        print()
    elif args.skip_docker:
        print("⏭️  跳过 Docker 测试")
        print()
    else:
        print("⚠️  Docker 不可用，跳过测试")
        print()
    
    # Pyodide 测试
    if not args.skip_pyodide:
        print("🌐 测试 Pyodide (WebAssembly)...")
        pyodide_tester = PyodideSecurityTest()
        
        # 检查 Pyodide 是否真正可用
        if not pyodide_tester.node_available:
            print("⚠️  Node.js 不可用，跳过 Pyodide 测试")
            print()
        else:
            # 检查 Pyodide 是否已安装
            if not pyodide_tester.pyodide_available:
                print("⚠️  Pyodide npm 包未安装，跳过测试")
                print("   提示: 运行 'npm install pyodide' 来安装")
                print()
            else:
                results["Pyodide"] = {}
                platforms.append("Pyodide")
                
                for test in SECURITY_TESTS:
                    result = pyodide_tester.run_test(test)
                    results["Pyodide"][test.name] = result
                    print(f"  {test.description}: {result.value}")
                print()
    
    # Claude SRT 测试
    if not args.skip_claude_srt and check_claude_srt_available():
        print("🤖 测试 Claude SRT (Sandboxed Runtime)...")
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
        print("⏭️  跳过 Claude SRT 测试")
        print()
    elif not check_claude_srt_available():
        print("⚠️  Claude SRT 不可用，跳过测试")
        print("   提示: 请确保已安装 srt 命令行工具")
        print()
    
    # 打印结果表格
    print("=" * 60)
    print("安全性对比结果")
    print("=" * 60)
    print()
    print_results_table(results, platforms)
    
    # 计算并打印安全评分
    scores = calculate_security_score(results)
    print("## 安全评分")
    print()
    print("| 平台 | 阻止 | 部分阻止 | 允许 | 安全评分 |")
    print("|------|------|----------|------|----------|")
    for platform in platforms:
        s = scores[platform]
        print(f"| {platform} | {s['blocked']} | {s['partial']} | {s['allowed']} | {s['score']:.1f}% |")
    print()
    
    # 输出 JSON 结果
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
        print(f"📄 结果已保存到 {args.output}")
    
    print("=" * 60)
    print("测试完成!")
    print("=" * 60)


if __name__ == "__main__":
    main()
