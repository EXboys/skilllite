#!/usr/bin/env python3
"""
SkillLite 性能基准测试: Skillbox (Rust 沙箱) vs Docker vs 原生 Python

测试维度：
1. 冷启动时间 - 从启动到执行第一行代码的时间
2. 代码执行时间 - 运行相同代码的总时间
3. 并发性能 - 同时启动多个实例的表现
"""

import time
import subprocess
import statistics
import json
import os
import tempfile
import shutil
from concurrent.futures import ThreadPoolExecutor, as_completed


def check_command_available(command: str) -> bool:
    """检查命令是否可用"""
    return shutil.which(command) is not None


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
    # 优先使用指定路径
    if binary_path and os.path.exists(binary_path):
        try:
            subprocess.run([binary_path, "--help"], capture_output=True, timeout=10)
            return True, binary_path
        except Exception:
            pass
    
    # 检查系统 PATH
    system_path = shutil.which("skillbox")
    if system_path:
        return True, system_path
    
    # 检查项目目录
    project_paths = [
        "./skillbox/target/release/skillbox",
        "../skillbox/target/release/skillbox",
        os.path.expanduser("~/.cargo/bin/skillbox"),
    ]
    for path in project_paths:
        if os.path.exists(path):
            return True, path
    
    return False, ""


class SkillboxBenchmark:
    """SkillLite Rust 沙箱 (skillbox) 性能测试"""
    
    def __init__(self, binary_path: str, work_dir: str = None):
        self.binary_path = binary_path
        self.work_dir = work_dir or tempfile.mkdtemp(prefix="skillbox_bench_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """创建测试用的 Skill 目录结构"""
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        # 创建 SKILL.md
        skill_md = """---
name: test-skill
description: Benchmark test skill
version: 1.0.0
entry_point: scripts/main.py
---
# Test Skill
"""
        with open(os.path.join(self.skill_dir, "SKILL.md"), "w") as f:
            f.write(skill_md)
    
    def _create_test_script(self, code: str) -> str:
        """创建测试脚本并返回路径"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(code)
        return script_path
    
    def measure_startup(self, iterations: int = 10) -> list:
        """测量启动时间（执行最简单的代码）"""
        times = []
        self._create_test_script('import json; print(json.dumps({"result": "hello"}))')
        input_json = '{}'  # 空的输入 JSON
        
        for i in range(iterations):
            start = time.perf_counter()
            result = subprocess.run(
                [self.binary_path, "run", self.skill_dir, input_json],
                capture_output=True,
                timeout=30,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            elapsed = (end - start) * 1000
            times.append(elapsed)
            
            if result.returncode != 0 and i == 0:
                print(f"    警告: skillbox 返回非零退出码: {result.returncode}")
                stderr = result.stderr.decode() if result.stderr else ""
                if stderr:
                    print(f"    stderr: {stderr[:200]}")
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 10) -> list:
        """测量代码执行时间"""
        times = []
        self._create_test_script(code)
        input_json = '{}'
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                [self.binary_path, "run", self.skill_dir, input_json],
                capture_output=True,
                timeout=60,
                cwd=self.work_dir
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def measure_concurrent(self, num_concurrent: int = 5, iterations: int = 3) -> dict:
        """测量并发执行性能"""
        self._create_test_script('import json; print(json.dumps({"result": "concurrent test"}))')
        input_json = '{}'
        
        def run_once():
            start = time.perf_counter()
            subprocess.run(
                [self.binary_path, "run", self.skill_dir, input_json],
                capture_output=True,
                timeout=30,
                cwd=self.work_dir
            )
            return (time.perf_counter() - start) * 1000
        
        all_times = []
        for _ in range(iterations):
            with ThreadPoolExecutor(max_workers=num_concurrent) as executor:
                futures = [executor.submit(run_once) for _ in range(num_concurrent)]
                batch_times = [f.result() for f in as_completed(futures)]
                all_times.extend(batch_times)
        
        return {
            "mean": statistics.mean(all_times),
            "max": max(all_times),
            "total_runs": len(all_times),
        }
    
    def cleanup(self):
        """清理临时目录"""
        if self.work_dir and os.path.exists(self.work_dir) and "skillbox_bench_" in self.work_dir:
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DockerBenchmark:
    """Docker 性能测试"""
    
    def __init__(self, image: str = "python:3.11-slim"):
        self.image = image
        self._ensure_image()
    
    def _ensure_image(self):
        """确保镜像存在"""
        print(f"  正在检查 Docker 镜像 {self.image}...")
        result = subprocess.run(
            ["docker", "images", "-q", self.image],
            capture_output=True,
            timeout=30
        )
        if not result.stdout.strip():
            print(f"  正在拉取镜像...")
            subprocess.run(["docker", "pull", self.image], capture_output=True, timeout=300)
    
    def measure_startup(self, iterations: int = 10) -> list:
        """测量容器启动时间"""
        times = []
        test_code = 'print("hello")'
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                ["docker", "run", "--rm", self.image, "python", "-c", test_code],
                capture_output=True,
                timeout=60
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 10) -> list:
        """测量代码执行时间"""
        times = []
        
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                ["docker", "run", "--rm", self.image, "python", "-c", code],
                capture_output=True,
                timeout=120
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        
        return times
    
    def measure_concurrent(self, num_concurrent: int = 5, iterations: int = 3) -> dict:
        """测量并发执行性能"""
        def run_once():
            start = time.perf_counter()
            subprocess.run(
                ["docker", "run", "--rm", self.image, "python", "-c", 'print("concurrent")'],
                capture_output=True,
                timeout=60
            )
            return (time.perf_counter() - start) * 1000
        
        all_times = []
        for _ in range(iterations):
            with ThreadPoolExecutor(max_workers=num_concurrent) as executor:
                futures = [executor.submit(run_once) for _ in range(num_concurrent)]
                batch_times = [f.result() for f in as_completed(futures)]
                all_times.extend(batch_times)
        
        return {
            "mean": statistics.mean(all_times),
            "max": max(all_times),
            "total_runs": len(all_times),
        }


class NativePythonBenchmark:
    """原生 Python 性能测试（作为基准参照）"""
    
    def __init__(self):
        self.python_path = shutil.which("python3") or shutil.which("python")
    
    def measure_startup(self, iterations: int = 10) -> list:
        """测量原生 Python 启动时间"""
        times = []
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                [self.python_path, "-c", 'print("hello")'],
                capture_output=True,
                timeout=30
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        return times
    
    def measure_execution(self, code: str, iterations: int = 10) -> list:
        """测量代码执行时间"""
        times = []
        for _ in range(iterations):
            start = time.perf_counter()
            subprocess.run(
                [self.python_path, "-c", code],
                capture_output=True,
                timeout=60
            )
            end = time.perf_counter()
            times.append((end - start) * 1000)
        return times


def print_comparison_results(results: dict, test_cases: dict):
    """打印对比结果"""
    print("\n" + "=" * 70)
    print("  对比结果汇总")
    print("=" * 70)
    
    has_skillbox = bool(results.get("skillbox"))
    has_docker = bool(results.get("docker"))
    has_native = bool(results.get("native_python"))
    
    # 表头
    header = f"{'测试项':<20}"
    if has_native:
        header += f"{'Python (ms)':<14}"
    if has_skillbox:
        header += f"{'Skillbox (ms)':<14}"
    if has_docker:
        header += f"{'Docker (ms)':<14}"
    if has_skillbox and has_docker:
        header += f"{'Skillbox 优势':<14}"
    
    print(f"\n{header}")
    print("-" * len(header))
    
    # 数据行
    all_tests = ["startup"] + list(test_cases.keys())
    for test_name in all_tests:
        row = f"{test_name:<20}"
        
        native_time = results["native_python"].get(test_name, {}).get("mean", 0) if has_native else 0
        skillbox_time = results["skillbox"].get(test_name, {}).get("mean", 0) if has_skillbox else 0
        docker_time = results["docker"].get(test_name, {}).get("mean", 0) if has_docker else 0
        
        if has_native and native_time:
            row += f"{native_time:<14.2f}"
        elif has_native:
            row += f"{'-':<14}"
            
        if has_skillbox and skillbox_time:
            row += f"{skillbox_time:<14.2f}"
        elif has_skillbox:
            row += f"{'-':<14}"
            
        if has_docker and docker_time:
            row += f"{docker_time:<14.2f}"
        elif has_docker:
            row += f"{'-':<14}"
        
        if has_skillbox and has_docker and skillbox_time and docker_time:
            speedup = docker_time / skillbox_time
            row += f"{speedup:.1f}x 更快"
        
        print(row)
    
    # 关键结论
    print("\n" + "-" * 70)
    print("📊 关键结论:")
    
    if has_skillbox and has_native:
        skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
        native_startup = results["native_python"].get("startup", {}).get("mean", 0)
        if skillbox_startup and native_startup:
            overhead = skillbox_startup - native_startup
            overhead_pct = (overhead / native_startup) * 100 if native_startup else 0
            print(f"  • Skillbox 沙箱开销: +{overhead:.1f} ms (+{overhead_pct:.0f}%)")
    
    if has_skillbox and has_docker:
        skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
        docker_startup = results["docker"].get("startup", {}).get("mean", 0)
        if skillbox_startup and docker_startup:
            speedup = docker_startup / skillbox_startup
            print(f"  • Skillbox vs Docker 启动速度: {speedup:.1f}x 更快")
            print(f"  • Docker 启动时间: {docker_startup:.0f} ms")
            print(f"  • Skillbox 启动时间: {skillbox_startup:.0f} ms")


def save_results(results: dict):
    """保存结果到 JSON 文件"""
    output_file = "benchmark_results.json"
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\n📁 详细结果已保存到: {output_file}")


def run_benchmark_suite(skillbox_binary: str = None, docker_image: str = "python:3.11-slim", iterations: int = 10):
    """运行完整的基准测试套件"""
    
    print("=" * 70)
    print("  SkillLite 性能基准测试")
    print("  Skillbox (Rust 沙箱) vs Docker vs 原生 Python")
    print("=" * 70)
    
    # 检查可用的测试环境
    docker_available = check_docker_available()
    skillbox_available, skillbox_path = check_skillbox_available(skillbox_binary)
    
    print("\n[环境检测]")
    print("-" * 50)
    print(f"  Skillbox (Rust 沙箱): {'✓ 可用 (' + skillbox_path + ')' if skillbox_available else '✗ 不可用'}")
    print(f"  Docker:               {'✓ 可用' if docker_available else '✗ 不可用'}")
    print(f"  原生 Python:          ✓ 可用 (作为基准参照)")
    
    if not skillbox_available and not docker_available:
        print("\n⚠️  警告: Skillbox 和 Docker 都不可用")
        print("  将仅运行原生 Python 基准测试作为参照")
        print("\n  要进行完整对比测试，请确保:")
        print("    1. 编译 skillbox: cd skillbox && cargo build --release")
        print("    2. 或安装 Docker: https://docs.docker.com/get-docker/")
    
    # 测试用例 - Skillbox 需要 JSON 输出，所以使用 json.dumps
    test_cases = {
        "simple_print": 'import json; print(json.dumps({"result": "Hello, World!"}))',
        "loop_1000": 'import json; print(json.dumps({"result": sum(range(1000))}))',
        "loop_100000": 'import json; print(json.dumps({"result": sum(range(100000))}))',
        "string_ops": 'import json; print(json.dumps({"result": len("hello" * 1000)}))',
        "list_comprehension": 'import json; print(json.dumps({"result": len([x**2 for x in range(1000)])}))',
        "fibonacci": '''
import json
def fib(n):
    if n <= 1: return n
    return fib(n-1) + fib(n-2)
print(json.dumps({"result": fib(20)}))
''',
    }
    
    results = {"skillbox": {}, "docker": {}, "native_python": {}}
    
    # 原生 Python 测试（作为基准）
    print("\n[原生 Python 测试] (无沙箱，作为性能基准)")
    print("-" * 50)
    native_bench = NativePythonBenchmark()
    
    print(f"  测试启动时间 ({iterations} 次)...")
    native_startup = native_bench.measure_startup(iterations)
    results["native_python"]["startup"] = {
        "mean": statistics.mean(native_startup),
        "min": min(native_startup),
        "max": max(native_startup),
    }
    print(f"    平均启动时间: {results['native_python']['startup']['mean']:.2f} ms")
    
    for name, code in test_cases.items():
        print(f"  测试 {name}...")
        exec_times = native_bench.measure_execution(code, iterations)
        results["native_python"][name] = {
            "mean": statistics.mean(exec_times),
            "min": min(exec_times),
            "max": max(exec_times),
        }
    
    # Skillbox 测试
    skillbox_bench = None
    if skillbox_available:
        print("\n[Skillbox 测试] (Rust 原生沙箱)")
        print("-" * 50)
        skillbox_bench = SkillboxBenchmark(skillbox_path)
        
        print(f"  测试启动时间 ({iterations} 次)...")
        try:
            skillbox_startup = skillbox_bench.measure_startup(iterations)
            results["skillbox"]["startup"] = {
                "mean": statistics.mean(skillbox_startup),
                "min": min(skillbox_startup),
                "max": max(skillbox_startup),
            }
            print(f"    平均启动时间: {results['skillbox']['startup']['mean']:.2f} ms")
            
            for name, code in test_cases.items():
                print(f"  测试 {name}...")
                exec_times = skillbox_bench.measure_execution(code, iterations)
                results["skillbox"][name] = {
                    "mean": statistics.mean(exec_times),
                    "min": min(exec_times),
                    "max": max(exec_times),
                }
            
            # 并发测试
            print(f"  测试并发性能 (5 并发)...")
            concurrent_result = skillbox_bench.measure_concurrent(num_concurrent=5, iterations=2)
            results["skillbox"]["concurrent_5"] = concurrent_result
            print(f"    平均执行时间: {concurrent_result['mean']:.2f} ms")
            
        except Exception as e:
            print(f"    ❌ Skillbox 测试失败: {e}")
    else:
        print("\n[跳过 Skillbox 测试]")
        print("  请先编译: cd skillbox && cargo build --release")
    
    # Docker 测试
    if docker_available:
        print("\n[Docker 测试]")
        print("-" * 50)
        docker_bench = DockerBenchmark(docker_image)
        
        print(f"  测试启动时间 ({iterations} 次)...")
        docker_startup = docker_bench.measure_startup(iterations)
        results["docker"]["startup"] = {
            "mean": statistics.mean(docker_startup),
            "min": min(docker_startup),
            "max": max(docker_startup),
        }
        print(f"    平均启动时间: {results['docker']['startup']['mean']:.2f} ms")
        
        for name, code in test_cases.items():
            print(f"  测试 {name}...")
            exec_times = docker_bench.measure_execution(code, iterations)
            results["docker"][name] = {
                "mean": statistics.mean(exec_times),
                "min": min(exec_times),
                "max": max(exec_times),
            }
        
        # 并发测试
        print(f"  测试并发性能 (5 并发)...")
        concurrent_result = docker_bench.measure_concurrent(num_concurrent=5, iterations=2)
        results["docker"]["concurrent_5"] = concurrent_result
        print(f"    平均执行时间: {concurrent_result['mean']:.2f} ms")
    else:
        print("\n[跳过 Docker 测试 - Docker 未安装]")
    
    # 清理
    if skillbox_bench:
        skillbox_bench.cleanup()
    
    # 输出对比结果
    print_comparison_results(results, test_cases)
    
    # 保存结果
    save_results(results)
    
    return results


def measure_skillbox_cold_start(skillbox_path: str, iterations: int = 5):
    """测量 Skillbox 冷启动时间（清除系统缓存后首次执行）"""
    print("\n[Skillbox 冷启动测试]")
    print("-" * 50)
    
    # 创建临时测试目录
    work_dir = tempfile.mkdtemp(prefix="skillbox_cold_")
    skill_dir = os.path.join(work_dir, "test-skill")
    scripts_dir = os.path.join(skill_dir, "scripts")
    os.makedirs(scripts_dir, exist_ok=True)
    
    # 创建测试文件
    with open(os.path.join(skill_dir, "SKILL.md"), "w") as f:
        f.write("---\nname: test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
    with open(os.path.join(scripts_dir, "main.py"), "w") as f:
        f.write('import json; print(json.dumps({"result": "cold start"}))')
    
    times = []
    
    for i in range(iterations):
        # 尝试清除文件系统缓存（需要 sudo，可能失败）
        subprocess.run(["sync"], capture_output=True)
        subprocess.run(["sudo", "purge"], capture_output=True, timeout=10)
        
        start = time.perf_counter()
        subprocess.run(
            [skillbox_path, "run", skill_dir, "{}"],
            capture_output=True,
            timeout=30,
            cwd=work_dir
        )
        end = time.perf_counter()
        times.append((end - start) * 1000)
    
    # 清理
    shutil.rmtree(work_dir, ignore_errors=True)
    
    print(f"  平均冷启动时间: {statistics.mean(times):.2f} ms")
    print(f"  最快: {min(times):.2f} ms")
    print(f"  最慢: {max(times):.2f} ms")
    
    return times


def measure_docker_cold_start(image: str = "python:3.11-slim", iterations: int = 3):
    """测量 Docker 真正的冷启动时间（每次都删除镜像重新拉取）"""
    print("\n" + "=" * 70)
    print("  冷启动对比测试")
    print("  Skillbox vs Docker（每次删除镜像后重新拉取）")
    print("=" * 70)
    
    # 先测试 Skillbox 冷启动
    skillbox_available, skillbox_path = check_skillbox_available()
    skillbox_times = []
    if skillbox_available:
        skillbox_times = measure_skillbox_cold_start(skillbox_path, iterations=5)
    
    # Docker 冷启动测试
    print("\n[Docker 冷启动测试]")
    print("-" * 50)
    print("  ⚠️  这个测试会比较慢，因为需要重新下载镜像")
    
    docker_times = []
    
    for i in range(iterations):
        print(f"\n  第 {i+1}/{iterations} 次冷启动测试...")
        
        # 1. 删除镜像
        print("    删除镜像...")
        subprocess.run(["docker", "rmi", "-f", image], capture_output=True, timeout=60)
        
        # 2. 清理 Docker 缓存
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True, timeout=60)
        
        # 3. 测量冷启动时间（包括拉取镜像 + 启动容器 + 执行代码）
        print("    开始冷启动计时（包括拉取镜像）...")
        start = time.perf_counter()
        result = subprocess.run(
            ["docker", "run", "--rm", image, "python", "-c", 'import json; print(json.dumps({"result": "cold start"}))'],
            capture_output=True,
            timeout=600  # 10分钟超时
        )
        end = time.perf_counter()
        
        elapsed = (end - start) * 1000
        docker_times.append(elapsed)
        print(f"    冷启动时间: {elapsed:.0f} ms ({elapsed/1000:.1f} 秒)")
    
    # 输出对比结果
    print("\n" + "=" * 70)
    print("📊 冷启动对比结果:")
    print("=" * 70)
    
    if skillbox_times:
        skillbox_avg = statistics.mean(skillbox_times)
        print(f"\n  Skillbox:")
        print(f"    • 平均冷启动时间: {skillbox_avg:.0f} ms")
        print(f"    • 范围: {min(skillbox_times):.0f} - {max(skillbox_times):.0f} ms")
    
    docker_avg = statistics.mean(docker_times)
    print(f"\n  Docker:")
    print(f"    • 平均冷启动时间: {docker_avg:.0f} ms ({docker_avg/1000:.1f} 秒)")
    print(f"    • 范围: {min(docker_times):.0f} - {max(docker_times):.0f} ms")
    
    if skillbox_times:
        speedup = docker_avg / skillbox_avg
        print(f"\n  🚀 结论:")
        print(f"    • Skillbox 比 Docker 冷启动快 {speedup:.0f}x")
        print(f"    • Docker 需要下载 ~150MB 镜像，Skillbox 是本地二进制")
    
    return {"skillbox": skillbox_times, "docker": docker_times}


if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(
        description="SkillLite 性能基准测试: Skillbox (Rust 沙箱) vs Docker vs 原生 Python",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例:
  # 自动检测 skillbox，跳过 Docker（如果未安装）
  python3 benchmark_comparison.py

  # 指定 skillbox 路径
  python3 benchmark_comparison.py --skillbox ./skillbox/target/release/skillbox

  # 完整测试（需要 Docker）
  python3 benchmark_comparison.py --iterations 20

  # 使用不同的 Docker 镜像
  python3 benchmark_comparison.py --docker-image python:3.12-alpine

  # 测试 Docker 真正的冷启动（会删除镜像重新拉取，较慢）
  python3 benchmark_comparison.py --cold-start --iterations 3
"""
    )
    parser.add_argument(
        "--skillbox", 
        type=str, 
        default=None,
        help="Skillbox 可执行文件路径（默认自动检测）"
    )
    parser.add_argument(
        "--docker-image", 
        type=str, 
        default="python:3.11-slim", 
        help="Docker 镜像名称（默认: python:3.11-slim）"
    )
    parser.add_argument(
        "--iterations", 
        type=int, 
        default=10, 
        help="每个测试的迭代次数（默认: 10）"
    )
    parser.add_argument(
        "--cold-start",
        action="store_true",
        help="测试 Docker 真正的冷启动（每次删除镜像重新拉取）"
    )
    
    args = parser.parse_args()
    
    if args.cold_start:
        # 冷启动测试模式
        if not check_docker_available():
            print("错误: Docker 未安装或未运行")
            exit(1)
        measure_docker_cold_start(args.docker_image, args.iterations)
    else:
        # 正常基准测试
        run_benchmark_suite(
            skillbox_binary=args.skillbox,
            docker_image=args.docker_image,
            iterations=args.iterations
        )
