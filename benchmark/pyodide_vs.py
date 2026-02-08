#!/usr/bin/env python3
"""
SkillLite Performance Benchmark: Skillbox vs Pyodide (WebAssembly)

Pyodide is the Python sandbox solution used by frameworks like LangChain,
running Python based on WebAssembly in browsers or Node.js.

Test dimensions:
1. Cold Start Latency - loading Pyodide runtime
2. Code Execution Time - total time to run the same code
"""

import time
import subprocess
import statistics
import json
import os
import tempfile
import shutil


def check_node_available() -> bool:
    """Check if Node.js is available"""
    return shutil.which("node") is not None


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


class PyodideBenchmark:
    """Pyodide (WebAssembly) Performance Test"""
    
    def __init__(self):
        self.work_dir = tempfile.mkdtemp(prefix="pyodide_bench_")
        self._setup_test_script()
    
    def _setup_test_script(self):
        """Create Node.js test script"""
        # Create a Node.js script that uses Pyodide
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

        # Create package.json
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
        """Install Pyodide npm package"""
        print("  Installing Pyodide (first time requires downloading ~50MB)...")
        result = subprocess.run(
            ["npm", "install"],
            cwd=self.work_dir,
            capture_output=True,
            timeout=300
        )
        return result.returncode == 0
    
    def measure_cold_start(self, iterations: int = 3) -> list:
        """Measure cold start latency (reload Pyodide each time)"""
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
            
            # Try to parse output for detailed timing
            if result.returncode == 0:
                try:
                    output = json.loads(result.stdout.decode().strip())
                    print(f"    Iteration {i+1}: Total time {total_time:.0f}ms (loading {output.get('load_time_ms', 0):.0f}ms)")
                except:
                    print(f"    Iteration {i+1}: {total_time:.0f}ms")
            else:
                print(f"    Iteration {i+1}: {total_time:.0f}ms (execution failed)")
        
        return times
    
    def measure_execution(self, code: str, iterations: int = 5) -> list:
        """Measure code execution time"""
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
        """Clean up temporary directory"""
        if self.work_dir and os.path.exists(self.work_dir):
            shutil.rmtree(self.work_dir, ignore_errors=True)


class SkillboxBenchmark:
    """Skillbox Performance Test"""
    
    def __init__(self, binary_path: str):
        self.binary_path = binary_path
        self.work_dir = tempfile.mkdtemp(prefix="skillbox_bench_")
        self._setup_test_skill()
    
    def _setup_test_skill(self):
        """Create Skill directory structure for testing"""
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
        """Measure startup time"""
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
        """Measure code execution time"""
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
    """Run Skillbox vs Pyodide comparison test"""

    print("=" * 70)
    print("  SkillLite Performance Benchmark")
    print("  Skillbox (Rust Sandbox) vs Pyodide (WebAssembly)")
    print("=" * 70)

    # Check environment
    node_available = check_node_available()
    skillbox_available, skillbox_path = check_skillbox_available()

    print("\n[Environment Check]")
    print("-" * 50)
    print(f"  Skillbox: {'✓ Available (' + skillbox_path + ')' if skillbox_available else '✗ Not available'}")
    print(f"  Node.js:  {'✓ Available' if node_available else '✗ Not available (Pyodide requires Node.js)'}")

    if not node_available:
        print("\n⚠️  Node.js is required to test Pyodide")
        print("  Install via: brew install node")
        return

    results = {"skillbox": {}, "pyodide": {}}

    # Test cases
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
    
    # Skillbox Test
    if skillbox_available:
        print("\n[Skillbox Test] (Rust Native Sandbox)")
        print("-" * 50)
        skillbox_bench = SkillboxBenchmark(skillbox_path)

        print("  Testing startup time (10 iterations)...")
        startup_times = skillbox_bench.measure_startup(10)
        results["skillbox"]["startup"] = {
            "mean": statistics.mean(startup_times),
            "min": min(startup_times),
            "max": max(startup_times),
        }
        print(f"    average: {results['skillbox']['startup']['mean']:.2f} ms")

        for name, code in test_cases.items():
            print(f"  Testing {name}...")
            exec_times = skillbox_bench.measure_execution(code, 5)
            results["skillbox"][name] = {
                "mean": statistics.mean(exec_times),
                "min": min(exec_times),
                "max": max(exec_times),
            }
        
        skillbox_bench.cleanup()
    
    # Pyodide Test
    print("\n[Pyodide Test] (WebAssembly)")
    print("-" * 50)
    pyodide_bench = PyodideBenchmark()
    
    if not pyodide_bench.install_dependencies():
        print("  ❌ Pyodide installation failed")
        return

    print("  Testing cold start latency (3 iterations)...")
    startup_times = pyodide_bench.measure_cold_start(3)
    results["pyodide"]["startup"] = {
        "mean": statistics.mean(startup_times),
        "min": min(startup_times),
        "max": max(startup_times),
    }
    print(f"    average: {results['pyodide']['startup']['mean']:.0f} ms")
    
    for name, code in test_cases.items():
        print(f"  Testing {name}...")
        exec_times = pyodide_bench.measure_execution(code, 3)
        results["pyodide"][name] = {
            "mean": statistics.mean(exec_times),
            "min": min(exec_times),
            "max": max(exec_times),
        }
    
    pyodide_bench.cleanup()

    # Output comparison results
    print("\n" + "=" * 70)
    print("  Comparison Results Summary")
    print("=" * 70)

    print(f"\n{'Test Item':<20} {'Skillbox (ms)':<15} {'Pyodide (ms)':<15} {'Skillbox Advantage':<15}")
    print("-" * 65)

    for test_name in ["startup"] + list(test_cases.keys()):
        skillbox_time = results["skillbox"].get(test_name, {}).get("mean", 0)
        pyodide_time = results["pyodide"].get(test_name, {}).get("mean", 0)

        if skillbox_time and pyodide_time:
            speedup = pyodide_time / skillbox_time
            print(f"{test_name:<20} {skillbox_time:<15.2f} {pyodide_time:<15.0f} {speedup:.0f}x faster")

    print("\n" + "-" * 70)
    print("📊 Key Conclusions:")

    skillbox_startup = results["skillbox"].get("startup", {}).get("mean", 0)
    pyodide_startup = results["pyodide"].get("startup", {}).get("mean", 0)

    if skillbox_startup and pyodide_startup:
        speedup = pyodide_startup / skillbox_startup
        print(f"  • Skillbox startup time: {skillbox_startup:.0f} ms")
        print(f"  • Pyodide startup time: {pyodide_startup:.0f} ms (needs to load ~50MB WebAssembly)")
        print(f"  • Skillbox is {speedup:.0f}x faster than Pyodide")

    # Save results
    script_dir = os.path.dirname(os.path.abspath(__file__))
    output_file = os.path.join(script_dir, "pyodide_results.json")
    os.makedirs(os.path.dirname(output_file), exist_ok=True)
    with open(output_file, "w") as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    print(f"\n📁 Detailed results saved to: {output_file}")


if __name__ == "__main__":
    run_benchmark()
