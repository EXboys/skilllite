#!/usr/bin/env python3
"""
SkillLite Performance Benchmark: SkillBox (Rust Sandbox) vs Docker vs Native Python

Test Dimensions:
1. Cold Start Time - Time from startup to executing the first line of code
2. Code Execution Time - Total time to run the same code
3. Concurrent Performance - Performance when starting multiple instances simultaneously
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
    """Check if a command is available"""
    return shutil.which(command) is not None


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


def check_skillbox_available(binary_path: str = None) -> tuple:
    """Check if skillbox is available, returns (available, path)"""
    # Prefer to use the specified path
    if binary_path and os.path.exists(binary_path):
        try:
            subprocess.run([binary_path, "--help"], capture_output=True, timeout=10)
            return True, binary_path
        except Exception:
            pass
    
    # Check system PATH
    system_path = shutil.which("skillbox")
    if system_path:
        return True, system_path
    
    # Check project directories
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
    """SkillLite Rust Sandbox (skillbox) Performance Test"""
    
    def __init__(self, binary_path: str, work_dir: str = None):
        self.binary_path = binary_path
        self.work_dir = work_dir or tempfile.mkdtemp(prefix="skillbox_bench_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """Create test Skill directory structure"""
        self.skill_dir = os.path.join(self.work_dir, "test-skill")
        scripts_dir = os.path.join(self.skill_dir, "scripts")
        os.makedirs(scripts_dir, exist_ok=True)
        
        # Create SKILL.md
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
        """Create test script and return path"""
        script_path = os.path.join(self.skill_dir, "scripts", "main.py")
        with open(script_path, "w") as f:
            f.write(code)
        return script_path
    
    def measure_startup(self, iterations: int = 10) -> list:
        """Measure startup time (execute simplest code)"""
        times = []
        self._create_test_script('import json; print(json.dumps({"result": "hello"}))')
        input_json = '{}'  # Empty input JSON
        
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
                print(f"    Warning: skillbox returned non-zero exit code: {result.returncode}")
                stderr = result.stderr.decode() if result.stderr else ""
                if stderr:
                    print(f"    stderr: {stderr[:200]}")
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 10) -> list:
        """Measure code execution time"""
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
        """Measure concurrent execution performance"""
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
        """Clean up temporary directory"""
        if self.work_dir and os.path.exists(self.work_dir) and "skillbox_bench_" in self.work_dir:
            shutil.rmtree(self.work_dir, ignore_errors=True)


class DockerBenchmark:
    """Docker Performance Test"""
    
    def __init__(self, image: str = "python:3.11-slim"):
        self.image = image
        self._ensure_image()
    
    def _ensure_image(self):
        """Ensure Docker image exists"""
        print(f"  Checking Docker image {self.image}...")
        result = subprocess.run(
            ["docker", "images", "-q", self.image],
            capture_output=True,
            timeout=30
        )
        if not result.stdout.strip():
            print(f"  Pulling image...")
            subprocess.run(["docker", "pull", self.image], capture_output=True, timeout=300)
    
    def measure_startup(self, iterations: int = 10) -> list:
        """Measure container startup time"""
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
        """Measure code execution time"""
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
        """Measure concurrent execution performance"""
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
    """Native Python performance test (baseline reference)"""

    def __init__(self):
        self.python_path = shutil.which("python3") or shutil.which("python")

    def measure_startup(self, iterations: int = 10) -> list:
        """Measure Native Python startup time"""
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
        """Measure code execution time"""
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
    """Print comparison results"""
    print("\n" + "=" * 70)
    print("  Comparison Results Summary")
    print("=" * 70)
    
    has_skillbox = bool(results.get("skillbox"))
    has_docker = bool(results.get("docker"))
    has_native = bool(results.get("native_python"))
    
    # Table header
    header = f"{'Test Item':<20}"
    if has_native:
        header += f"{'Python (ms)':<14}"
    if has_skillbox:
        header += f"{'Skillbox (ms)':<14}"
    if has_docker:
        header += f"{'Docker (ms)':<14}"
    if has_skillbox and has_docker:
        header += f"{'Skillbox Advantage':<14}"
    
    print(f"\n{header}")
    print("-" * len(header))
    
    # Data rows
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
            row += f"{speedup:.1f}x faster"
        
        print(row)
    
    # Key conclusions
    print("\n" + "-" * 70)
    print("📊 Key Conclusions:")
    
    if has_skillbox and has_native:
        skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
        native_startup = results["native_python"].get("startup", {}).get("mean", 0)
        if skillbox_startup and native_startup:
            overhead = skillbox_startup - native_startup
            overhead_pct = (overhead / native_startup) * 100 if native_startup else 0
            print(f"  • SkillBox Sandbox Overhead: +{overhead:.1f} ms (+{overhead_pct:.0f}%)")
    
    if has_skillbox and has_docker:
        skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
        docker_startup = results["docker"].get("startup", {}).get("mean", 0)
        if skillbox_startup and docker_startup:
            speedup = docker_startup / skillbox_startup
            print(f"  • SkillBox vs Docker Startup Speed: {speedup:.1f}x faster")
            print(f"  • Docker Startup Time: {docker_startup:.0f} ms")
            print(f"  • SkillBox Startup Time: {skillbox_startup:.0f} ms")


def save_results(results: dict):
    """Save results to JSON file"""
    output_file = "benchmark_results.json"
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\n📁 Detailed results saved to: {output_file}")


def run_benchmark_suite(skillbox_binary: str = None, docker_image: str = "python:3.11-slim", iterations: int = 10):
    """Run complete benchmark test suite"""
    
    print("=" * 70)
    print("  SkillLite Performance Benchmark")
    print("  SkillBox (Rust Sandbox) vs Docker vs Native Python")
    print("=" * 70)
    
    # Check available test environments
    docker_available = check_docker_available()
    skillbox_available, skillbox_path = check_skillbox_available(skillbox_binary)

    print("\n[Environment Detection]")
    print("-" * 50)
    print(f"  SkillBox (Rust Sandbox): {'✓ Available (' + skillbox_path + ')' if skillbox_available else '✗ Not Available'}")
    print(f"  Docker:               {'✓ Available' if docker_available else '✗ Not Available'}")
    print(f"  Native Python:          ✓ Available (baseline reference)")

    if not skillbox_available and not docker_available:
        print("\n⚠️  Warning: Both Skillbox and Docker are not available")
        print("  Will only run Native Python benchmark as reference")
        print("\n  To run complete comparison tests, please ensure:")
        print("    1. Compile skillbox: cd skillbox && cargo build --release")
        print("    2. Or install Docker: https://docs.docker.com/get-docker/")
    
    # Test cases - SkillBox requires JSON output, so using json.dumps
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
    
    # Native Python test (as baseline)
    print("\n[Native Python Test] (no sandbox, baseline reference)")
    print("-" * 50)
    native_bench = NativePythonBenchmark()
    
    print(f"  Testing startup time ({iterations} iterations)...")
    native_startup = native_bench.measure_startup(iterations)
    results["native_python"]["startup"] = {
        "mean": statistics.mean(native_startup),
        "min": min(native_startup),
        "max": max(native_startup),
    }
    print(f"    Average startup time: {results['native_python']['startup']['mean']:.2f} ms")
    
    for name, code in test_cases.items():
        print(f"  Testing {name}...")
        exec_times = native_bench.measure_execution(code, iterations)
        results["native_python"][name] = {
            "mean": statistics.mean(exec_times),
            "min": min(exec_times),
            "max": max(exec_times),
        }
    
    # SkillBox test
    skillbox_bench = None
    if skillbox_available:
        print("\n[Skillbox Test] (Rust Native Sandbox)")
        print("-" * 50)
        skillbox_bench = SkillboxBenchmark(skillbox_path)
        
        print(f"  Testing startup time ({iterations} iterations)...")
        try:
            skillbox_startup = skillbox_bench.measure_startup(iterations)
            results["skillbox"]["startup"] = {
                "mean": statistics.mean(skillbox_startup),
                "min": min(skillbox_startup),
                "max": max(skillbox_startup),
            }
            print(f"    Average startup time: {results['skillbox']['startup']['mean']:.2f} ms")
            
            for name, code in test_cases.items():
                print(f"  Testing {name}...")
                exec_times = skillbox_bench.measure_execution(code, iterations)
                results["skillbox"][name] = {
                    "mean": statistics.mean(exec_times),
                    "min": min(exec_times),
                    "max": max(exec_times),
                }
            
            # concurrent test
            print(f"  Testing concurrent performance (5 concurrent)...")
            concurrent_result = skillbox_bench.measure_concurrent(num_concurrent=5, iterations=2)
            results["skillbox"]["concurrent_5"] = concurrent_result
            print(f"    Average execution time: {concurrent_result['mean']:.2f} ms")
            
        except Exception as e:
            print(f"    ❌ SkillBox test failed: {e}")
    else:
        print("\n[Skip SkillBox Test]")
        print("  Please compile first: cd skillbox && cargo build --release")
    
    # Docker test
    if docker_available:
        print("\n[Docker Test]")
        print("-" * 50)
        docker_bench = DockerBenchmark(docker_image)
        
        print(f"  Testing startup time ({iterations} iterations)...")
        docker_startup = docker_bench.measure_startup(iterations)
        results["docker"]["startup"] = {
            "mean": statistics.mean(docker_startup),
            "min": min(docker_startup),
            "max": max(docker_startup),
        }
        print(f"    Average startup time: {results['docker']['startup']['mean']:.2f} ms")
        
        for name, code in test_cases.items():
            print(f"  Testing {name}...")
            exec_times = docker_bench.measure_execution(code, iterations)
            results["docker"][name] = {
                "mean": statistics.mean(exec_times),
                "min": min(exec_times),
                "max": max(exec_times),
            }
        
        # concurrent test
        print(f"  Testing concurrent performance (5 concurrent)...")
        concurrent_result = docker_bench.measure_concurrent(num_concurrent=5, iterations=2)
        results["docker"]["concurrent_5"] = concurrent_result
        print(f"    Average execution time: {concurrent_result['mean']:.2f} ms")
    else:
        print("\n[Skip Docker Test - Docker not installed]")
    
    # Cleanup
    if skillbox_bench:
        skillbox_bench.cleanup()
    
    # Output comparison results
    print_comparison_results(results, test_cases)
    
    # Save results
    save_results(results)
    
    return results


def measure_skillbox_cold_start(skillbox_path: str, iterations: int = 5):
    """Measure Skillbox cold start time (first execution after clearing system cache)"""
    print("\n[SkillBox Cold Start Test]")
    print("-" * 50)
    
    # Create temporary test directory
    work_dir = tempfile.mkdtemp(prefix="skillbox_cold_")
    skill_dir = os.path.join(work_dir, "test-skill")
    scripts_dir = os.path.join(skill_dir, "scripts")
    os.makedirs(scripts_dir, exist_ok=True)
    
    # Create test files
    with open(os.path.join(skill_dir, "SKILL.md"), "w") as f:
        f.write("---\nname: test\nversion: 1.0.0\nentry_point: scripts/main.py\n---\n")
    with open(os.path.join(scripts_dir, "main.py"), "w") as f:
        f.write('import json; print(json.dumps({"result": "cold start"}))')
    
    times = []
    
    for i in range(iterations):
        # Try to clear file system cache (requires sudo, may fail)
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
    
    # Cleanup
    shutil.rmtree(work_dir, ignore_errors=True)
    
    print(f"  Avg Cold Start Time: {statistics.mean(times):.2f} ms")
    print(f"  Fastest: {min(times):.2f} ms")
    print(f"  Slowest: {max(times):.2f} ms")
    
    return times


def measure_docker_cold_start(image: str = "python:3.11-slim", iterations: int = 3):
    """Measure Docker cold start time (delete image before each run)"""
    print("\n" + "=" * 70)
    print("  Cold Start Comparison Test")
    print("  SkillBox vs Docker (re-pull image after each deletion)")
    print("=" * 70)

    # First test Skillbox cold start
    skillbox_available, skillbox_path = check_skillbox_available()
    skillbox_times = []
    if skillbox_available:
        skillbox_times = measure_skillbox_cold_start(skillbox_path, iterations=5)
    
    # Docker cold start test
    print("\n[Docker Cold Start Test]")
    print("-" * 50)
    print("  ⚠️  This test will be slower because it needs to re-download the image")
    
    docker_times = []
    
    for i in range(iterations):
        print(f"\n  Cold start test {i+1}/{iterations}...")

        # 1. Delete image
        print("    Deleting image...")
        subprocess.run(["docker", "rmi", "-f", image], capture_output=True, timeout=60)

        # 2. Clean Docker cache
        subprocess.run(["docker", "system", "prune", "-f"], capture_output=True, timeout=60)

        # 3. Measure cold start time (including pulling image + starting container + executing code)
        print("    Starting cold start measurement (including pulling image)...")
        start = time.perf_counter()
        result = subprocess.run(
            ["docker", "run", "--rm", image, "python", "-c", 'import json; print(json.dumps({"result": "cold start"}))'],
            capture_output=True,
            timeout=600  # 10 minutes timeout
        )
        end = time.perf_counter()
        
        elapsed = (end - start) * 1000
        docker_times.append(elapsed)
        print(f"    Cold Start Time: {elapsed:.0f} ms ({elapsed/1000:.1f} seconds)")
    
    # Output comparison results
    print("\n" + "=" * 70)
    print("📊 Cold Start Comparison Results:")
    print("=" * 70)
    
    if skillbox_times:
        skillbox_avg = statistics.mean(skillbox_times)
        print(f"\n  Skillbox:")
        print(f"    • Avg Cold Start Time: {skillbox_avg:.0f} ms")
        print(f"    • Range: {min(skillbox_times):.0f} - {max(skillbox_times):.0f} ms")
    
    docker_avg = statistics.mean(docker_times)
    print(f"\n  Docker:")
    print(f"    • Avg Cold Start Time: {docker_avg:.0f} ms ({docker_avg/1000:.1f} seconds)")
    print(f"    • Range: {min(docker_times):.0f} - {max(docker_times):.0f} ms")
    
    if skillbox_times:
        speedup = docker_avg / skillbox_avg
        print(f"\n  🚀 Conclusion:")
        print(f"    • Skillbox cold start is {speedup:.0f}x")
        print(f"    • Docker needs to download ~150MB image, SkillBox is a local binary")
    
    return {"skillbox": skillbox_times, "docker": docker_times}


if __name__ == "__main__":
    import argparse
    
    parser = argparse.ArgumentParser(
        description="SkillLite Performance Benchmark: SkillBox (Rust Sandbox) vs Docker vs Native Python",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Auto-detect skillbox, skip Docker（if not installed）
  python3 benchmark_comparison.py

  # Specify skillbox path
  python3 benchmark_comparison.py --skillbox ./skillbox/target/release/skillbox

  # Full test (requires Docker)
  python3 benchmark_comparison.py --iterations 20

  # Use different Docker image
  python3 benchmark_comparison.py --docker-image python:3.12-alpine

  # Test Docker true cold start (will delete image and re-pull, slower)
  python3 benchmark_comparison.py --cold-start --iterations 3
"""
    )
    parser.add_argument(
        "--skillbox",
        type=str,
        default=None,
        help="SkillBox executable path (auto-detect by default)"
    )
    parser.add_argument(
        "--docker-image",
        type=str,
        default="python:3.11-slim",
        help="Docker image name (default: python:3.11-slim)"
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=10,
        help="Number of iterations for each test (default: 10)"
    )
    parser.add_argument(
        "--cold-start",
        action="store_true",
        help="Test Docker true cold start (delete image and re-pull each iteration)"
    )
    
    args = parser.parse_args()
    
    if args.cold_start:
        # Cold Start Test mode
        if not check_docker_available():
            print("Error: Docker not installed or not running")
            exit(1)
        measure_docker_cold_start(args.docker_image, args.iterations)
    else:
        # Normal benchmark test
        run_benchmark_suite(
            skillbox_binary=args.skillbox,
            docker_image=args.docker_image,
            iterations=args.iterations
        )
