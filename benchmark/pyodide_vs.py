#!/usr/bin/env python3
"""
SkillLite 性能基准测试: Skillbox vs Pyodide (WebAssembly)

Pyodide 是 LangChain 等框架使用的 Python 沙箱方案，
基于 WebAssembly 在浏览器或 Node.js 中运行 Python。

测试维度：
1. 冷启动时间 - 加载 Pyodide 运行时
2. 代码执行时间 - 运行相同代码的总时间
"""

import time
import subprocess
import statistics
import json
import os
import tempfile
import shutil


def check_node_available() -> bool:
    """检查 Node.js 是否可用"""
    return shutil.which("node") is not None


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


class PyodideBenchmark:
    """Pyodide (WebAssembly) 性能测试"""
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="pyodide_bench_")
        self._setup_test_script()
    
    def _setup_test_script(self):
        """创建 Node.js 测试脚本"""
        # 创建一个使用 Pyodide 的 Node.js 脚本
        self.test_script = os.path.join(self.work_dir, "pyodide_test.mjs")
        
        script_content = '''
import { loadPyodide } from "pyodide";

async function runPython(code) {
    const startLoad = performance.now();
    const pyodide = await loadPyodide();
    const loadTime = performance.now() - startLoad;
    
    const startExec = performance.now();
    const result = await pyodide.runPythonAsync(code);
    const execTime = performance.now() - startExec;
    
    console.log(JSON.stringify({
        load_time_ms: loadTime,
        exec_time_ms: execTime,
        total_time_ms: loadTime + execTime,
        result: result
    }));
}

const code = process.argv[2] || 'print("hello")';
runPython(code).catch(console.error);
'''
        with open(self.test_script, "w") as f:
            f.write(script_content)
        
        # 创建 package.json
        package_json = os.path.join(self.work_dir, "package.json")
        with open(package_json, "w") as f:
            json.dump({
                "name": "pyodide-benchmark",
                "type": "module",
                "dependencies": {
                    "pyodide": "^0.26.0"
                }
            }, f)
    
    def install_dependencies(self):
        """安装 Pyodide npm 包"""
        print("  正在安装 Pyodide (首次需要下载 ~50MB)...")
        result = subprocess.run(
            ["npm", "install"],
            cwd=self.work_dir,
            capture_output=True,
            timeout=300
        )
        return result.returncode == 0
    
    def measure_cold_start(self, iterations: int = 3) -> list:
        """测量冷启动时间（每次都重新加载 Pyodide）"""
        times = []
        
        for i in range(iterations):
            start = time.perf_counter()
            result = subprocess.run(
                ["node", self.test_script, 'import json; print(json.dumps({"result": "hello"}))'],
                cwd=self.work_dir,
                capture_output=True,
                timeout=120
            )
            end = time.perf_counter()
            
            total_time = (end - start) * 1000
            times.append(total_time)
            
            # 尝试解析输出获取详细时间
            if result.returncode == 0:
                try:
                    output = json.loads(result.stdout.decode().strip())
                    print(f"    第 {i+1} 次: 总时间 {total_time:.0f}ms (加载 {output.get('load_time_ms', 0):.0f}ms)")
                except:
                    print(f"    第 {i+1} 次: {total_time:.0f}ms")
            else:
                print(f"    第 {i+1} 次: {total_time:.0f}ms (执行失败)")
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 5) -> list:
        """测量代码执行时间"""
        times = []
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                ["node", self.test_script, code],
                cwd=self.work_dir,
                capture_output=True,
                timeout=120
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def cleanup(self):
        """清理临时目录"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class SkillboxBenchmark:
    """Skillbox 性能测试"""
    
    def __init__(self, binary_path: str):
        self.binary_path = binary_path
        self.work_dir = tempfile.mkdtemp(prefix="skillbox_bench_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """创建测试用的 Skill 目录结构"""
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        with open(os.path.join(self.skill_dir, "SKILL.md"), "w") as f:
            f.write("---\nname: test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
    
    def _create_test_script(self, code: str):
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(code)
    
    def measure_startup(self, iterations: int = 10) -> list:
        """测量启动时间"""
        times = []
        self._create_test_script('import json; print(json.dumps({"result": "hello"}))')
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                [self.binary_path, "run", self.skill_dir, "{}"],
                capture_output=True,
                timeout=30,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 10) -> list:
        """测量代码执行时间"""
        times = []
        self._create_test_script(code)
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                [self.binary_path, "run", self.skill_dir, "{}"],
                capture_output=True,
                timeout=60,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def cleanup(self):
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


def run_benchmark():
    """运行 Skillbox vs Pyodide 对比测试"""
    
    print("=" * 70)
    print("  SkillLite 性能基准测试")
    print("  Skillbox (Rust 沙箱) vs Pyodide (WebAssembly)")
    print("=" * 70)
    
    # 检查环境
    node_available = check_node_available()
    skillbox_available, skillbox_path = check_skillbox_available()
    
    print("\n[环境检测]")
    print("-" * 50)
    print(f"  Skillbox: {'✓ 可用 (' + skillbox_path + ')' if skillbox_available else '✗ 不可用'}")
    print(f"  Node.js:  {'✓ 可用' if node_available else '✗ 不可用 (Pyodide 需要 Node.js)'}")
    
    if not node_available:
        print("\n⚠️  需要安装 Node.js 才能测试 Pyodide")
        print("  安装方法: brew install node")
        return
    
    results = {"skillbox": {}, "pyodide": {}}
    
    # 测试用例
    test_cases = {
        "simple_print": 'import json; print(json.dumps({"result": "Hello"}))',
        "loop_1000": 'import json; print(json.dumps({"result": sum(range(1000))}))',
        "fibonacci": '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
print(json.dumps({"result": fib(20)}))
''',
    }
    
    # Skillbox 测试
    if skillbox_available:
        print("\n[Skillbox 测试] (Rust 原生沙箱)")
        print("-" * 50)
        skillbox_bench = SkillboxBenchmark(skillbox_path)
        
        print("  测试启动时间 (10 次)...")
        startup_times = skillbox_bench.measure_startup(10)
        results["skillbox"]["startup"] = {
            "mean": statistics.mean(startup_times),
            "min": min(startup_times),
            "max": max(startup_times),
        }
        print(f"    平均: {results['skillbox']['startup']['mean']:.2f} ms")
        
        for name, code in test_cases.items():
            print(f"  测试 {name}...")
            exec_times = skillbox_bench.measure_execution(code, 5)
            results["skillbox"][name] = {
                "mean": statistics.mean(exec_times),
                "min": min(exec_times),
                "max": max(exec_times),
            }
        
        skillbox_bench.cleanup()
    
    # Pyodide 测试
    print("\n[Pyodide 测试] (WebAssembly)")
    print("-" * 50)
    pyodide_bench = PyodideBenchmark()
    
    if not pyodide_bench.install_dependencies():
        print("  ❌ Pyodide 安装失败")
        return
    
    print("  测试冷启动时间 (3 次)...")
    startup_times = pyodide_bench.measure_cold_start(3)
    results["pyodide"]["startup"] = {
        "mean": statistics.mean(startup_times),
        "min": min(startup_times),
        "max": max(startup_times),
    }
    print(f"    平均: {results['pyodide']['startup']['mean']:.0f} ms")
    
    for name, code in test_cases.items():
        print(f"  测试 {name}...")
        exec_times = pyodide_bench.measure_execution(code, 3)
        results["pyodide"][name] = {
            "mean": statistics.mean(exec_times),
            "min": min(exec_times),
            "max": max(exec_times),
        }
    
    pyodide_bench.cleanup()
    
    # 输出对比结果
    print("\n" + "=" * 70)
    print("  对比结果汇总")
    print("=" * 70)
    
    print(f"\n{'测试项':<20} {'Skillbox (ms)':<15} {'Pyodide (ms)':<15} {'Skillbox 优势':<15}")
    print("-" * 65)
    
    for test_name in ["startup"] + list(test_cases.keys()):
        skillbox_time = results["skillbox"].get(test_name, {}).get("mean", 0)
        pyodide_time = results["pyodide"].get(test_name, {}).get("mean", 0)
        
        if skillbox_time and pyodide_time:
            speedup = pyodide_time / skillbox_time
            print(f"{test_name:<20} {skillbox_time:<15.2f} {pyodide_time:<15.0f} {speedup:.0f}x 更快")
    
    print("\n" + "-" * 70)
    print("📊 关键结论:")
    
    skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
    pyodide_startup = results["pyodide"].get("startup", {}).get("mean", 0)
    
    if skillbox_startup and pyodide_startup:
        speedup = pyodide_startup / skillbox_startup
        print(f"  • Skillbox 启动时间: {skillbox_startup:.0f} ms")
        print(f"  • Pyodide 启动时间: {pyodide_startup:.0f} ms (需加载 ~50MB WebAssembly)")
        print(f"  • Skillbox 比 Pyodide 快 {speedup:.0f}x")
    
    # 保存结果
    output_file = "benchmark/pyodide_results.json"
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\n📁 详细结果已保存到: {output_file}")


if __name__ == "__main__":
    run_benchmark()
