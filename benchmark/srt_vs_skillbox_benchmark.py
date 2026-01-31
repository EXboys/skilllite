#!/usr/bin/env python3
"""
SkillLite 性能基准测试: Skillbox vs Claude Code Sandbox (srt)

真实对比测试：
- Skillbox: Rust + Seatbelt (macOS) / Namespace+Seccomp (Linux)
- srt (Claude Code Sandbox): Node.js/TypeScript + Seatbelt (macOS) / bubblewrap (Linux)

参考: https://www.anthropic.com/engineering/claude-code-sandboxing
"""

import time
import subprocess
import statistics
import json
import os
import tempfile
import shutil
import sys
import platform
from dataclasses import dataclass
from typing import Optional

@dataclass
class BenchmarkResult:
    """基准测试结果"""
    name: str
    times_ms: list
    success: bool
    output: str = ""
    error: str = ""
    memory_kb: float = 0  # 峰值内存使用 (KB)
    
    @property
    def mean(self) -> float:
        return statistics.mean(self.times_ms) if self.times_ms else 0
    
    @property
    def min(self) -> float:
        return min(self.times_ms) if self.times_ms else 0
    
    @property
    def max(self) -> float:
        return max(self.times_ms) if self.times_ms else 0
    
    @property
    def stdev(self) -> float:
        return statistics.stdev(self.times_ms) if len(self.times_ms) > 1 else 0


class ResourceMonitor:
    """资源监控器 - 测量进程的内存消耗"""
    
    @staticmethod
    def get_peak_memory_kb(command: list, cwd: str = None, timeout: int = 30) -> tuple:
        """
        运行命令并获取峰值内存使用量
        返回: (elapsed_ms, success, stdout, stderr, peak_memory_kb)
        """
        is_macos = platform.system() == "Darwin"
        
        if is_macos:
            # macOS: 使用 /usr/bin/time -l
            full_command = ["/usr/bin/time", "-l"] + command
            start = time.perf_counter()
            try:
                result = subprocess.run(
                    full_command,
                    capture_output=True,
                    timeout=timeout,
                    cwd=cwd
                )
                end = time.perf_counter()
                elapsed_ms = (end - start) * 1000
                
                stderr_text = result.stderr.decode(errors='replace')
                # macOS time 输出格式: "maximum resident set size" 单位是字节
                memory_kb = 0
                for line in stderr_text.split('\n'):
                    if 'maximum resident set size' in line.lower():
                        try:
                            # 提取数字 (字节)
                            parts = line.strip().split()
                            memory_bytes = int(parts[0])
                            memory_kb = memory_bytes / 1024
                        except (ValueError, IndexError):
                            pass
                        break
                
                return (
                    elapsed_ms,
                    result.returncode == 0,
                    result.stdout.decode(errors='replace'),
                    stderr_text,
                    memory_kb
                )
            except subprocess.TimeoutExpired:
                return (timeout * 1000, False, "", "Timeout", 0)
            except Exception as e:
                return (0, False, "", str(e), 0)
        else:
            # Linux: 使用 /usr/bin/time -v
            full_command = ["/usr/bin/time", "-v"] + command
            start = time.perf_counter()
            try:
                result = subprocess.run(
                    full_command,
                    capture_output=True,
                    timeout=timeout,
                    cwd=cwd
                )
                end = time.perf_counter()
                elapsed_ms = (end - start) * 1000
                
                stderr_text = result.stderr.decode(errors='replace')
                # Linux time 输出格式: "Maximum resident set size (kbytes):"
                memory_kb = 0
                for line in stderr_text.split('\n'):
                    if 'maximum resident set size' in line.lower():
                        try:
                            parts = line.strip().split(':')
                            memory_kb = float(parts[-1].strip())
                        except (ValueError, IndexError):
                            pass
                        break
                
                return (
                    elapsed_ms,
                    result.returncode == 0,
                    result.stdout.decode(errors='replace'),
                    stderr_text,
                    memory_kb
                )
            except subprocess.TimeoutExpired:
                return (timeout * 1000, False, "", "Timeout", 0)
            except Exception as e:
                return (0, False, "", str(e), 0)


class SrtBenchmark:
    """Claude Code Sandbox (srt) 性能测试"""
    
    def __init__(self):
        self.srt_path = shutil.which("srt")
        if not self.srt_path:
            raise RuntimeError("srt not found in PATH")
        self.work_dir = tempfile.mkdtemp(prefix="srt_bench_")
        self.resource_monitor = ResourceMonitor()
    
    def run_command(self, command: str, timeout: int = 30) -> tuple:
        """运行 srt 命令并返回 (耗时ms, 成功, stdout, stderr)"""
        start = time.perf_counter()
        try:
            result = subprocess.run(
                ["srt"] + command.split(),
                capture_output=True,
                timeout=timeout,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            elapsed_ms = (end - start) * 1000
            return (
                elapsed_ms,
                result.returncode == 0,
                result.stdout.decode(errors='replace'),
                result.stderr.decode(errors='replace')
            )
        except subprocess.TimeoutExpired:
            return (timeout * 1000, False, "", "Timeout")
        except Exception as e:
            return (0, False, "", str(e))
    
    def run_command_with_memory(self, command: list, timeout: int = 30) -> tuple:
        """运行命令并测量内存，返回 (耗时ms, 成功, stdout, stderr, memory_kb)"""
        return self.resource_monitor.get_peak_memory_kb(
            ["srt"] + command,
            cwd=self.work_dir,
            timeout=timeout
        )
    
    def run_python_code(self, code: str, timeout: int = 30) -> tuple:
        """通过 srt 运行 Python 代码"""
        script_path = os.path.join(self.work_dir, "test_script.py")
        with open(script_path, "w") as f:
            f.write(code)
        
        start = time.perf_counter()
        try:
            result = subprocess.run(
                ["srt", "python3", script_path],
                capture_output=True,
                timeout=timeout,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            elapsed_ms = (end - start) * 1000
            return (
                elapsed_ms,
                result.returncode == 0,
                result.stdout.decode(errors='replace'),
                result.stderr.decode(errors='replace')
            )
        except subprocess.TimeoutExpired:
            return (timeout * 1000, False, "", "Timeout")
        except Exception as e:
            return (0, False, "", str(e))
    
    def run_python_code_with_memory(self, code: str, timeout: int = 30) -> tuple:
        """通过 srt 运行 Python 代码并测量内存"""
        script_path = os.path.join(self.work_dir, "test_script.py")
        with open(script_path, "w") as f:
            f.write(code)
        
        return self.resource_monitor.get_peak_memory_kb(
            ["srt", "python3", script_path],
            cwd=self.work_dir,
            timeout=timeout
        )
    
    def measure_startup(self, iterations: int = 10) -> BenchmarkResult:
        """测量启动时间 (echo hello)"""
        times = []
        last_output = ""
        last_error = ""
        success = True
        
        for _ in range(iterations):
            elapsed, ok, stdout, stderr = self.run_command("echo hello")
            times.append(elapsed)
            last_output = stdout
            last_error = stderr
            if not ok:
                success = False
        
        return BenchmarkResult("startup", times, success, last_output, last_error)
    
    def measure_startup_with_memory(self) -> BenchmarkResult:
        """测量启动时间和内存消耗"""
        elapsed, ok, stdout, stderr, memory_kb = self.run_command_with_memory(["echo", "hello"])
        return BenchmarkResult("startup", [elapsed], ok, stdout, stderr, memory_kb)
    
    def measure_python_execution(self, name: str, code: str, iterations: int = 10) -> BenchmarkResult:
        """测量 Python 代码执行时间"""
        times = []
        last_output = ""
        last_error = ""
        success = True
        
        for _ in range(iterations):
            elapsed, ok, stdout, stderr = self.run_python_code(code)
            times.append(elapsed)
            last_output = stdout
            last_error = stderr
            if not ok:
                success = False
        
        return BenchmarkResult(name, times, success, last_output, last_error)
    
    def measure_python_with_memory(self, name: str, code: str) -> BenchmarkResult:
        """测量 Python 代码执行时间和内存消耗"""
        elapsed, ok, stdout, stderr, memory_kb = self.run_python_code_with_memory(code)
        return BenchmarkResult(name, [elapsed], ok, stdout, stderr, memory_kb)
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class SkillboxBenchmark:
    """Skillbox 性能测试"""
    
    def __init__(self):
        self.skillbox_path = shutil.which("skillbox")
        if not self.skillbox_path:
            raise RuntimeError("skillbox not found in PATH")
        self.work_dir = tempfile.mkdtemp(prefix="skillbox_bench_")
        self.resource_monitor = ResourceMonitor()
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """创建测试 skill 目录结构"""
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        with open(os.path.join(self.skill_dir, "SKILL.md"), "w") as f:
            f.write("---\nname: test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
    
    def _create_test_script(self, code: str):
        """创建测试脚本"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(code)
    
    def run_skill(self, code: str, timeout: int = 30) -> tuple:
        """运行 skill 并返回 (耗时ms, 成功, stdout, stderr)"""
        self._create_test_script(code)
        
        start = time.perf_counter()
        try:
            result = subprocess.run(
                [self.skillbox_path, "run", self.skill_dir, "{}"],
                capture_output=True,
                timeout=timeout,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            elapsed_ms = (end - start) * 1000
            return (
                elapsed_ms,
                result.returncode == 0,
                result.stdout.decode(errors='replace'),
                result.stderr.decode(errors='replace')
            )
        except subprocess.TimeoutExpired:
            return (timeout * 1000, False, "", "Timeout")
        except Exception as e:
            return (0, False, "", str(e))
    
    def run_skill_with_memory(self, code: str, timeout: int = 30) -> tuple:
        """运行 skill 并测量内存，返回 (耗时ms, 成功, stdout, stderr, memory_kb)"""
        self._create_test_script(code)
        
        return self.resource_monitor.get_peak_memory_kb(
            [self.skillbox_path, "run", self.skill_dir, "{}"],
            cwd=self.work_dir,
            timeout=timeout
        )
    
    def measure_startup(self, iterations: int = 10) -> BenchmarkResult:
        """测量启动时间"""
        times = []
        code = 'import json; print(json.dumps({"result": "hello"}))'
        last_output = ""
        last_error = ""
        success = True
        
        for _ in range(iterations):
            elapsed, ok, stdout, stderr = self.run_skill(code)
            times.append(elapsed)
            last_output = stdout
            last_error = stderr
            if not ok:
                success = False
        
        return BenchmarkResult("startup", times, success, last_output, last_error)
    
    def measure_startup_with_memory(self) -> BenchmarkResult:
        """测量启动时间和内存消耗"""
        code = 'import json; print(json.dumps({"result": "hello"}))'
        elapsed, ok, stdout, stderr, memory_kb = self.run_skill_with_memory(code)
        return BenchmarkResult("startup", [elapsed], ok, stdout, stderr, memory_kb)
    
    def measure_python_execution(self, name: str, code: str, iterations: int = 10) -> BenchmarkResult:
        """测量 Python 代码执行时间"""
        times = []
        last_output = ""
        last_error = ""
        success = True
        
        for _ in range(iterations):
            elapsed, ok, stdout, stderr = self.run_skill(code)
            times.append(elapsed)
            last_output = stdout
            last_error = stderr
            if not ok:
                success = False
        
        return BenchmarkResult(name, times, success, last_output, last_error)
    
    def measure_python_with_memory(self, name: str, code: str) -> BenchmarkResult:
        """测量 Python 代码执行时间和内存消耗"""
        elapsed, ok, stdout, stderr, memory_kb = self.run_skill_with_memory(code)
        return BenchmarkResult(name, [elapsed], ok, stdout, stderr, memory_kb)
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class SecurityTest:
    """安全性测试"""
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="security_test_")
    
    def test_srt_security(self) -> dict:
        """测试 srt 的安全限制"""
        results = {}
        
        # 测试 1: 文件系统访问限制
        print("  测试文件系统访问限制...")
        result = subprocess.run(
            ["srt", "cat", "/etc/passwd"],
            capture_output=True,
            timeout=10
        )
        results["fs_read_etc_passwd"] = {
            "blocked": result.returncode != 0,
            "output": result.stdout.decode(errors='replace')[:200],
            "error": result.stderr.decode(errors='replace')[:200]
        }
        
        # 测试 2: 网络访问限制
        print("  测试网络访问限制...")
        result = subprocess.run(
            ["srt", "curl", "-s", "--connect-timeout", "5", "https://example.com"],
            capture_output=True,
            timeout=15
        )
        results["network_access"] = {
            "blocked": result.returncode != 0 or "blocked" in result.stderr.decode().lower(),
            "output": result.stdout.decode(errors='replace')[:200],
            "error": result.stderr.decode(errors='replace')[:200]
        }
        
        # 测试 3: 进程创建限制
        print("  测试进程创建...")
        result = subprocess.run(
            ["srt", "bash", "-c", "echo subprocess_test"],
            capture_output=True,
            timeout=10
        )
        results["process_creation"] = {
            "allowed": result.returncode == 0 and "subprocess_test" in result.stdout.decode(),
            "output": result.stdout.decode(errors='replace')[:200],
            "error": result.stderr.decode(errors='replace')[:200]
        }
        
        # 测试 4: 环境变量隔离
        print("  测试环境变量隔离...")
        result = subprocess.run(
            ["srt", "bash", "-c", "echo $HOME"],
            capture_output=True,
            timeout=10
        )
        results["env_isolation"] = {
            "home_visible": len(result.stdout.decode().strip()) > 0,
            "output": result.stdout.decode(errors='replace')[:200],
            "error": result.stderr.decode(errors='replace')[:200]
        }
        
        # 测试 5: 写入系统目录
        print("  测试写入系统目录限制...")
        result = subprocess.run(
            ["srt", "touch", "/tmp/srt_security_test_file"],
            capture_output=True,
            timeout=10
        )
        results["write_tmp"] = {
            "allowed": result.returncode == 0,
            "output": result.stdout.decode(errors='replace')[:200],
            "error": result.stderr.decode(errors='replace')[:200]
        }
        
        return results
    
    def test_skillbox_security(self) -> dict:
        """测试 skillbox 的安全限制"""
        results = {}
        skill_dir = os.path.join(self.work_dir, "security-skill")
        scripts_dir = os.path.join(skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        with open(os.path.join(skill_dir, "SKILL.md"), "w") as f:
            f.write("---\nname: security-test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
        
        def run_security_test(code: str) -> tuple:
            script_path = os.path.join(scripts_dir, "main.py")
            with open(script_path, "w") as f:
                f.write(code)
            result = subprocess.run(
                ["skillbox", "run", skill_dir, "{}"],
                capture_output=True,
                timeout=15
            )
            return result.returncode == 0, result.stdout.decode(errors='replace'), result.stderr.decode(errors='replace')
        
        # 测试 1: 文件系统访问限制
        print("  测试文件系统访问限制...")
        code = '''
import json
try:
    with open("/etc/passwd", "r") as f:
        content = f.read()[:100]
    print(json.dumps({"success": True, "content": content}))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
'''
        ok, stdout, stderr = run_security_test(code)
        results["fs_read_etc_passwd"] = {
            "blocked": not ok or '"success": false' in stdout.lower() or "error" in stdout.lower(),
            "output": stdout[:200],
            "error": stderr[:200]
        }
        
        # 测试 2: 网络访问限制
        print("  测试网络访问限制...")
        code = '''
import json
import urllib.request
try:
    with urllib.request.urlopen("https://example.com", timeout=5) as response:
        content = response.read()[:100].decode()
    print(json.dumps({"success": True, "content": content}))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
'''
        ok, stdout, stderr = run_security_test(code)
        results["network_access"] = {
            "blocked": not ok or '"success": false' in stdout.lower() or "error" in stdout.lower(),
            "output": stdout[:200],
            "error": stderr[:200]
        }
        
        # 测试 3: 进程创建限制
        print("  测试进程创建...")
        code = '''
import json
import subprocess
try:
    result = subprocess.run(["echo", "subprocess_test"], capture_output=True)
    print(json.dumps({"success": True, "output": result.stdout.decode()}))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
'''
        ok, stdout, stderr = run_security_test(code)
        results["process_creation"] = {
            "allowed": ok and "subprocess_test" in stdout,
            "output": stdout[:200],
            "error": stderr[:200]
        }
        
        # 测试 4: 环境变量隔离
        print("  测试环境变量隔离...")
        code = '''
import json
import os
print(json.dumps({"home": os.environ.get("HOME", ""), "path": os.environ.get("PATH", "")[:100]}))
'''
        ok, stdout, stderr = run_security_test(code)
        results["env_isolation"] = {
            "home_visible": "home" in stdout.lower() and len(stdout) > 20,
            "output": stdout[:200],
            "error": stderr[:200]
        }
        
        # 测试 5: 写入临时目录
        print("  测试写入临时目录...")
        code = '''
import json
try:
    with open("/tmp/skillbox_security_test", "w") as f:
        f.write("test")
    print(json.dumps({"success": True}))
except Exception as e:
    print(json.dumps({"success": False, "error": str(e)}))
'''
        ok, stdout, stderr = run_security_test(code)
        results["write_tmp"] = {
            "allowed": ok and '"success": true' in stdout.lower(),
            "output": stdout[:200],
            "error": stderr[:200]
        }
        
        return results
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)
        # 清理测试文件
        for f in ["/tmp/srt_security_test_file", "/tmp/skillbox_security_test"]:
            try:
                os.remove(f)
            except:
                pass


def print_header(title: str):
    """打印标题"""
    print("\n" + "=" * 70)
    print(f"  {title}")
    print("=" * 70)


def print_section(title: str):
    """打印章节标题"""
    print(f"\n[{title}]")
    print("-" * 50)


def run_benchmark():
    """运行完整的 benchmark 测试"""
    
    print_header("SkillLite 性能基准测试")
    print("  Skillbox (Rust) vs srt/Claude Code Sandbox (Node.js)")
    print("  " + "=" * 66)
    
    # 检查环境
    print_section("环境检测")
    
    srt_path = shutil.which("srt")
    skillbox_path = shutil.which("skillbox")
    is_macos = os.uname().sysname == "Darwin"
    
    print(f"  srt:       {'✓ ' + srt_path if srt_path else '✗ 不可用'}")
    print(f"  skillbox:  {'✓ ' + skillbox_path if skillbox_path else '✗ 不可用'}")
    print(f"  平台:      {'macOS (Seatbelt)' if is_macos else 'Linux'}")
    
    # 获取版本
    if srt_path:
        result = subprocess.run(["srt", "--version"], capture_output=True)
        srt_version = result.stdout.decode().strip()
        print(f"  srt 版本:  {srt_version}")
    
    if skillbox_path:
        result = subprocess.run(["skillbox", "--version"], capture_output=True)
        skillbox_version = result.stdout.decode().strip()
        print(f"  skillbox 版本: {skillbox_version}")
    
    if not srt_path or not skillbox_path:
        print("\n⚠️  需要同时安装 srt 和 skillbox 才能进行对比测试")
        return
    
    # 测试用例
    test_cases = {
        "simple_print": 'import json; print(json.dumps({"result": "Hello"}))',
        "loop_10000": 'import json; print(json.dumps({"result": sum(range(10000))}))',
        "fibonacci_25": '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
print(json.dumps({"result": fib(25)}))
''',
        "list_comprehension": '''
import json
result = [x**2 for x in range(1000)]
print(json.dumps({"result": len(result)}))
''',
        "dict_operations": '''
import json
d = {str(i): i**2 for i in range(1000)}
result = sum(d.values())
print(json.dumps({"result": result}))
''',
    }
    
    iterations = 10
    results = {"srt": {}, "skillbox": {}}
    memory_results = {"srt": {}, "skillbox": {}}
    
    # ==================== 性能测试 ====================
    print_header("性能测试")
    
    # srt 测试
    print_section("srt (Claude Code Sandbox) 测试")
    srt_bench = SrtBenchmark()
    
    print(f"  测试启动时间 ({iterations} 次)...")
    startup_result = srt_bench.measure_startup(iterations)
    results["srt"]["startup"] = {
        "mean": startup_result.mean,
        "min": startup_result.min,
        "max": startup_result.max,
        "stdev": startup_result.stdev,
        "success": startup_result.success
    }
    print(f"    平均: {startup_result.mean:.2f} ms (±{startup_result.stdev:.2f})")
    
    for name, code in test_cases.items():
        print(f"  测试 {name}...")
        exec_result = srt_bench.measure_python_execution(name, code, iterations)
        results["srt"][name] = {
            "mean": exec_result.mean,
            "min": exec_result.min,
            "max": exec_result.max,
            "stdev": exec_result.stdev,
            "success": exec_result.success
        }
        status = "✓" if exec_result.success else "✗"
        print(f"    {status} 平均: {exec_result.mean:.2f} ms")
    
    # srt 内存测试
    print_section("srt 内存消耗测试")
    print("  测试启动内存...")
    mem_result = srt_bench.measure_startup_with_memory()
    memory_results["srt"]["startup"] = mem_result.memory_kb
    print(f"    峰值内存: {mem_result.memory_kb:.2f} KB ({mem_result.memory_kb/1024:.2f} MB)")
    
    for name, code in test_cases.items():
        print(f"  测试 {name} 内存...")
        mem_result = srt_bench.measure_python_with_memory(name, code)
        memory_results["srt"][name] = mem_result.memory_kb
        print(f"    峰值内存: {mem_result.memory_kb:.2f} KB ({mem_result.memory_kb/1024:.2f} MB)")
    
    srt_bench.cleanup()
    
    # skillbox 测试
    print_section("Skillbox (Rust) 测试")
    skillbox_bench = SkillboxBenchmark()
    
    print(f"  测试启动时间 ({iterations} 次)...")
    startup_result = skillbox_bench.measure_startup(iterations)
    results["skillbox"]["startup"] = {
        "mean": startup_result.mean,
        "min": startup_result.min,
        "max": startup_result.max,
        "stdev": startup_result.stdev,
        "success": startup_result.success
    }
    print(f"    平均: {startup_result.mean:.2f} ms (±{startup_result.stdev:.2f})")
    
    for name, code in test_cases.items():
        print(f"  测试 {name}...")
        exec_result = skillbox_bench.measure_python_execution(name, code, iterations)
        results["skillbox"][name] = {
            "mean": exec_result.mean,
            "min": exec_result.min,
            "max": exec_result.max,
            "stdev": exec_result.stdev,
            "success": exec_result.success
        }
        status = "✓" if exec_result.success else "✗"
        print(f"    {status} 平均: {exec_result.mean:.2f} ms")
    
    # skillbox 内存测试
    print_section("Skillbox 内存消耗测试")
    print("  测试启动内存...")
    mem_result = skillbox_bench.measure_startup_with_memory()
    memory_results["skillbox"]["startup"] = mem_result.memory_kb
    print(f"    峰值内存: {mem_result.memory_kb:.2f} KB ({mem_result.memory_kb/1024:.2f} MB)")
    
    for name, code in test_cases.items():
        print(f"  测试 {name} 内存...")
        mem_result = skillbox_bench.measure_python_with_memory(name, code)
        memory_results["skillbox"][name] = mem_result.memory_kb
        print(f"    峰值内存: {mem_result.memory_kb:.2f} KB ({mem_result.memory_kb/1024:.2f} MB)")
    
    skillbox_bench.cleanup()
    
    # ==================== 安全性测试 ====================
    print_header("安全性测试")
    
    security_test = SecurityTest()
    
    print_section("srt 安全性测试")
    srt_security = security_test.test_srt_security()
    results["srt"]["security"] = srt_security
    
    print_section("Skillbox 安全性测试")
    skillbox_security = security_test.test_skillbox_security()
    results["skillbox"]["security"] = skillbox_security
    
    security_test.cleanup()
    
    # ==================== 结果汇总 ====================
    print_header("性能对比结果")
    
    print(f"\n{'测试项':<20} {'srt (ms)':<15} {'Skillbox (ms)':<15} {'对比':<20}")
    print("-" * 70)
    
    all_tests = ["startup"] + list(test_cases.keys())
    for test_name in all_tests:
        srt_time = results["srt"].get(test_name, {}).get("mean", 0)
        skillbox_time = results["skillbox"].get(test_name, {}).get("mean", 0)
        
        if srt_time and skillbox_time:
            if srt_time < skillbox_time:
                ratio = skillbox_time / srt_time
                comparison = f"srt 快 {ratio:.2f}x"
            else:
                ratio = srt_time / skillbox_time
                comparison = f"Skillbox 快 {ratio:.2f}x"
            print(f"{test_name:<20} {srt_time:<15.2f} {skillbox_time:<15.2f} {comparison}")
        else:
            print(f"{test_name:<20} {'N/A':<15} {'N/A':<15} {'无法对比'}")
    
    # ==================== 内存消耗对比 ====================
    print_header("内存消耗对比结果")
    
    print(f"\n{'测试项':<20} {'srt (MB)':<15} {'Skillbox (MB)':<15} {'对比':<25}")
    print("-" * 75)
    
    for test_name in all_tests:
        srt_mem_kb = memory_results["srt"].get(test_name, 0)
        skillbox_mem_kb = memory_results["skillbox"].get(test_name, 0)
        srt_mem_mb = srt_mem_kb / 1024
        skillbox_mem_mb = skillbox_mem_kb / 1024
        
        if srt_mem_kb > 0 and skillbox_mem_kb > 0:
            if srt_mem_kb < skillbox_mem_kb:
                ratio = skillbox_mem_kb / srt_mem_kb
                comparison = f"srt 省 {ratio:.2f}x"
            else:
                ratio = srt_mem_kb / skillbox_mem_kb
                comparison = f"Skillbox 省 {ratio:.2f}x"
            print(f"{test_name:<20} {srt_mem_mb:<15.2f} {skillbox_mem_mb:<15.2f} {comparison}")
        else:
            print(f"{test_name:<20} {'N/A':<15} {'N/A':<15} {'无法对比'}")
    
    # 安全性对比
    print_header("安全性对比结果")
    
    security_items = [
        ("fs_read_etc_passwd", "读取 /etc/passwd", "blocked"),
        ("network_access", "网络访问", "blocked"),
        ("process_creation", "进程创建", "allowed"),
        ("env_isolation", "环境变量隔离", "home_visible"),
        ("write_tmp", "写入 /tmp", "allowed"),
    ]
    
    print(f"\n{'安全项':<25} {'srt':<15} {'Skillbox':<15}")
    print("-" * 55)
    
    for key, name, check_field in security_items:
        srt_val = srt_security.get(key, {}).get(check_field, "N/A")
        skillbox_val = skillbox_security.get(key, {}).get(check_field, "N/A")
        
        srt_str = "✓ 是" if srt_val else "✗ 否" if srt_val is False else str(srt_val)
        skillbox_str = "✓ 是" if skillbox_val else "✗ 否" if skillbox_val is False else str(skillbox_val)
        
        print(f"{name:<25} {srt_str:<15} {skillbox_str:<15}")
    
    # 关键结论
    print_header("关键结论")
    
    srt_startup = results["srt"].get("startup", {}).get("mean", 0)
    skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
    
    print("\n📊 性能分析:")
    print(f"  • srt 启动时间: {srt_startup:.0f} ms")
    print(f"  • Skillbox 启动时间: {skillbox_startup:.0f} ms")
    
    if srt_startup and skillbox_startup:
        if srt_startup < skillbox_startup:
            ratio = skillbox_startup / srt_startup
            print(f"  • srt 启动速度比 Skillbox 快约 {ratio:.1f}x")
        else:
            ratio = srt_startup / skillbox_startup
            print(f"  • Skillbox 启动速度比 srt 快约 {ratio:.1f}x")
    
    # 内存分析
    srt_startup_mem = memory_results["srt"].get("startup", 0)
    skillbox_startup_mem = memory_results["skillbox"].get("startup", 0)
    
    print("\n💾 内存消耗分析:")
    print(f"  • srt 启动内存: {srt_startup_mem/1024:.2f} MB")
    print(f"  • Skillbox 启动内存: {skillbox_startup_mem/1024:.2f} MB")
    
    if srt_startup_mem > 0 and skillbox_startup_mem > 0:
        if srt_startup_mem < skillbox_startup_mem:
            ratio = skillbox_startup_mem / srt_startup_mem
            print(f"  • srt 内存占用比 Skillbox 少约 {ratio:.1f}x")
        else:
            ratio = srt_startup_mem / skillbox_startup_mem
            print(f"  • Skillbox 内存占用比 srt 少约 {ratio:.1f}x")
    
    print("\n🔒 安全性分析:")
    srt_fs_blocked = srt_security.get("fs_read_etc_passwd", {}).get("blocked", False)
    skillbox_fs_blocked = skillbox_security.get("fs_read_etc_passwd", {}).get("blocked", False)
    srt_net_blocked = srt_security.get("network_access", {}).get("blocked", False)
    skillbox_net_blocked = skillbox_security.get("network_access", {}).get("blocked", False)
    
    print(f"  • 文件系统隔离: srt={'✓' if srt_fs_blocked else '✗'}, Skillbox={'✓' if skillbox_fs_blocked else '✗'}")
    print(f"  • 网络隔离: srt={'✓' if srt_net_blocked else '✗'}, Skillbox={'✓' if skillbox_net_blocked else '✗'}")
    
    print("\n📝 技术栈对比:")
    print("  • srt: Node.js/TypeScript + Seatbelt (macOS) / bubblewrap (Linux)")
    print("  • Skillbox: Rust + Seatbelt (macOS) / Namespace+Seccomp (Linux)")
    
    # 保存结果
    all_results = {
        "performance": results,
        "memory": memory_results
    }
    output_file = "benchmark/srt_vs_skillbox_results.json"
    with open(output_file, "w") as f:
        json.dump(all_results, f, indent=2, ensure_ascii=False, default=str)
    print(f"\n📁 详细结果已保存到: {output_file}")


if __name__ == "__main__":
    run_benchmark()
