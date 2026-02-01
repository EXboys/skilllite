#!/usr/bin/env python3
"""
SkillBox Benchmark Runner - High Concurrency Performance Comparison Test

Comparing SkillBox with other sandbox solutions under high concurrency scenarios:
1. SkillBox Native Sandbox (Seatbelt/Namespace)
2. Docker Container Sandbox
3. Direct Execution (No Sandbox - Baseline)
4. Subprocess with Resource Limits
5. SRT (Anthropic Sandbox Runtime)
6. Pyodide (WebAssembly)

Test metrics:
- Cold Start Latency
- Warm Start Latency
- Throughput under Concurrency
- P50/P95/P99 Latency
- Resource Usage (CPU/Memory)
"""

import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional

# Project root directory
PROJECT_ROOT = Path(__file__).parent.parent
SKILLS_DIR = PROJECT_ROOT / ".skills"
CALCULATOR_SKILL = SKILLS_DIR / "calculator"

# SkillBox binary path
SKILLBOX_BIN = shutil.which("skillbox") or str(PROJECT_ROOT / "skillbox" / "target" / "release" / "skillbox")


@dataclass
class BenchmarkResult:
    """Single execution result"""
    executor_name: str
    success: bool
    latency_ms: float
    stdout: str = ""
    stderr: str = ""
    error: Optional[str] = None


@dataclass
class BenchmarkStats:
    """Statistics"""
    executor_name: str
    total_requests: int
    successful_requests: int
    failed_requests: int
    min_latency_ms: float
    max_latency_ms: float
    avg_latency_ms: float
    p50_latency_ms: float
    p95_latency_ms: float
    p99_latency_ms: float
    throughput_rps: float
    total_time_sec: float
    
    def to_dict(self) -> dict:
        return {
            "executor": self.executor_name,
            "total_requests": self.total_requests,
            "successful": self.successful_requests,
            "failed": self.failed_requests,
            "latency_ms": {
                "min": round(self.min_latency_ms, 2),
                "max": round(self.max_latency_ms, 2),
                "avg": round(self.avg_latency_ms, 2),
                "p50": round(self.p50_latency_ms, 2),
                "p95": round(self.p95_latency_ms, 2),
                "p99": round(self.p99_latency_ms, 2),
            },
            "throughput_rps": round(self.throughput_rps, 2),
            "total_time_sec": round(self.total_time_sec, 2),
        }


def percentile(data: List[float], p: float) -> float:
    """Calculate percentile"""
    if not data:
        return 0.0
    sorted_data = sorted(data)
    index = int(len(sorted_data) * p / 100)
    return sorted_data[min(index, len(sorted_data) - 1)]


class BaseExecutor:
    """Executor base class"""
    
    name: str = "base"
    
    def setup(self) -> None:
        pass
    
    def teardown(self) -> None:
        pass
    
    def execute(self, input_json: str) -> BenchmarkResult:
        raise NotImplementedError


class SkillBoxExecutor(BaseExecutor):
    """SkillBox native sandbox executor"""

    name = "SkillBox (Native Sandbox)"

    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, sandbox_level: Optional[int] = None):
        self.skill_dir = skill_dir
        self.skillbox_bin = SKILLBOX_BIN
        # Get sandbox level from environment variable or parameter, default is 3
        if sandbox_level is not None:
            self.sandbox_level = sandbox_level
        else:
            self.sandbox_level = int(os.environ.get("SKILLBOX_SANDBOX_LEVEL", "3"))
        # Update executor name to reflect security level
        self.name = f"SkillBox (Level {self.sandbox_level})"
        
    def setup(self) -> None:
        if not os.path.exists(self.skillbox_bin):
            raise RuntimeError(f"SkillBox binary not found at {self.skillbox_bin}")
    
    def execute(self, input_json: str) -> BenchmarkResult:
        start_time = time.perf_counter()
        try:
            # Set environment variable to pass sandbox level
            env = os.environ.copy()
            env["SKILLBOX_SANDBOX_LEVEL"] = str(self.sandbox_level)
            
            result = subprocess.run(
                [self.skillbox_bin, "run", str(self.skill_dir), input_json],
                capture_output=True,
                text=True,
                timeout=30,
                env=env
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            return BenchmarkResult(
                executor_name=self.name,
                success=result.returncode == 0,
                latency_ms=latency_ms,
                stdout=result.stdout,
                stderr=result.stderr,
                error=None if result.returncode == 0 else f"Exit code: {result.returncode}"
            )
        except subprocess.TimeoutExpired:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout"
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e)
            )



class DockerExecutor(BaseExecutor):
    """Docker container sandbox executor"""
    
    name = "Docker Container"
    
    def __init__(self, skill_dir: Path = CALCULATOR_SKILL):
        self.skill_dir = skill_dir
        self.image_name = "skillbox-benchmark-python"
        self.docker_available = False
        
    def setup(self) -> None:
        try:
            result = subprocess.run(
                ["docker", "version"],
                capture_output=True,
                timeout=5
            )
            self.docker_available = result.returncode == 0
        except (subprocess.TimeoutExpired, FileNotFoundError):
            self.docker_available = False
            print(f"[WARN] Docker not available, {self.name} will be skipped")
            return
            
        dockerfile_content = """FROM python:3.11-slim
WORKDIR /app
COPY scripts/main.py /app/main.py
CMD ["python", "/app/main.py"]
"""
        with tempfile.TemporaryDirectory() as tmpdir:
            dockerfile_path = Path(tmpdir) / "Dockerfile"
            dockerfile_path.write_text(dockerfile_content)
            
            scripts_dir = Path(tmpdir) / "scripts"
            scripts_dir.mkdir()
            shutil.copy(self.skill_dir / "scripts" / "main.py", scripts_dir / "main.py")
            
            result = subprocess.run(
                ["docker", "build", "-t", self.image_name, "."],
                cwd=tmpdir,
                capture_output=True,
                timeout=120
            )
            if result.returncode != 0:
                print(f"[WARN] Failed to build Docker image: {result.stderr.decode()}")
                self.docker_available = False
    
    def teardown(self) -> None:
        if self.docker_available:
            subprocess.run(
                ["docker", "rmi", "-f", self.image_name],
                capture_output=True
            )
    
    def execute(self, input_json: str) -> BenchmarkResult:
        if not self.docker_available:
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=0,
                error="Docker not available"
            )
            
        start_time = time.perf_counter()
        try:
            result = subprocess.run(
                [
                    "docker", "run", "--rm", "-i",
                    "--memory=512m",
                    "--cpus=1",
                    "--network=none",
                    "--security-opt=no-new-privileges",
                    self.image_name
                ],
                input=input_json,
                capture_output=True,
                text=True,
                timeout=30
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            return BenchmarkResult(
                executor_name=self.name,
                success=result.returncode == 0,
                latency_ms=latency_ms,
                stdout=result.stdout,
                stderr=result.stderr,
                error=None if result.returncode == 0 else f"Exit code: {result.returncode}"
            )
        except subprocess.TimeoutExpired:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout"
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e)
            )


class SubprocessResourceLimitExecutor(BaseExecutor):
    """Subprocess executor with resource limits"""
    
    name = "Subprocess (Resource Limits)"
    
    def __init__(self, script_path: Path = CALCULATOR_SKILL / "scripts" / "main.py"):
        self.script_path = script_path
        self.resource_available = False
        
    def setup(self) -> None:
        try:
            import resource
            resource.getrlimit(resource.RLIMIT_CPU)
            self.resource_available = True
        except (ImportError, OSError, ValueError):
            self.resource_available = False
            print(f"[WARN] Resource limits not available on this platform")
        
    def execute(self, input_json: str) -> BenchmarkResult:
        start_time = time.perf_counter()
        try:
            preexec_fn = None
            
            if self.resource_available:
                import resource
                
                def set_limits():
                    try:
                        resource.setrlimit(resource.RLIMIT_CPU, (5, 10))
                        resource.setrlimit(resource.RLIMIT_NOFILE, (256, 256))
                    except (OSError, ValueError):
                        pass
                
                preexec_fn = set_limits
            
            result = subprocess.run(
                [sys.executable, str(self.script_path)],
                input=input_json,
                capture_output=True,
                text=True,
                timeout=30,
                preexec_fn=preexec_fn
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            return BenchmarkResult(
                executor_name=self.name,
                success=result.returncode == 0,
                latency_ms=latency_ms,
                stdout=result.stdout,
                stderr=result.stderr,
                error=None if result.returncode == 0 else f"Exit code: {result.returncode}"
            )
        except subprocess.TimeoutExpired:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout"
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e)
            )


class SRTExecutor(BaseExecutor):
    """SRT (Sandbox Runtime) Executor - Open source sandbox tool by Anthropic

    SRT uses the same underlying technology stack as SkillBox:
    - macOS: Seatbelt (sandbox-exec)
    - Linux: bubblewrap + namespace

    Installation: npm install -g @anthropic-ai/sandbox-runtime
    """
    
    name = "SRT (Anthropic Sandbox)"
    
    def __init__(self, script_path: Path = CALCULATOR_SKILL / "scripts" / "main.py"):
        self.script_path = script_path
        self.srt_bin = None
        self.srt_available = False
        
    def setup(self) -> None:
        # First try which
        self.srt_bin = shutil.which("srt") or shutil.which("sandbox-runtime")

        if not self.srt_bin:
            # Try to find from npm global path
            try:
                npm_global = subprocess.run(
                    ["npm", "root", "-g"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if npm_global.returncode == 0:
                    npm_path = Path(npm_global.stdout.strip())
                    possible_paths = [
                        npm_path.parent / "bin" / "srt",
                        npm_path / "@anthropic-ai" / "sandbox-runtime" / "bin" / "srt",
                    ]
                    for p in possible_paths:
                        if p.exists():
                            self.srt_bin = str(p)
                            break
            except (subprocess.TimeoutExpired, FileNotFoundError):
                pass
        
        # Try common nvm paths
        if not self.srt_bin:
            home = Path.home()
            nvm_paths = list(home.glob(".nvm/versions/node/*/bin/srt"))
            if nvm_paths:
                self.srt_bin = str(nvm_paths[-1])  # Use latest version
        
        if self.srt_bin:
            self.srt_available = True
        else:
            print("[WARN] SRT not found. Install via: npm install -g @anthropic-ai/sandbox-runtime")
    
    def execute(self, input_json: str) -> BenchmarkResult:
        if not self.srt_available:
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=0,
                error="SRT not installed"
            )
            
        start_time = time.perf_counter()
        try:
            # SRT command format: srt [command...] (no need for run subcommand)
            result = subprocess.run(
                [self.srt_bin, sys.executable, str(self.script_path)],
                input=input_json,
                capture_output=True,
                text=True,
                timeout=30
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            return BenchmarkResult(
                executor_name=self.name,
                success=result.returncode == 0,
                latency_ms=latency_ms,
                stdout=result.stdout,
                stderr=result.stderr,
                error=None if result.returncode == 0 else f"Exit code: {result.returncode}"
            )
        except subprocess.TimeoutExpired:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout"
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e)
            )


class PyodideExecutor(BaseExecutor):
    """Pyodide (WebAssembly) Executor

    Pyodide compiles CPython to WebAssembly, running in a browser sandbox.
    According to official documentation, Pyodide is typically 3-5x slower than native Python.

    Install dependencies: cd benchmark && npm install
    """
    
    name = "Pyodide (WebAssembly)"
    
    def __init__(self, script_path: Path = CALCULATOR_SKILL / "scripts" / "main.py"):
        self.script_path = script_path
        self.pyodide_available = False
        self.node_available = False
        self.pyodide_runner = Path(__file__).parent / "pyodide_runner_template.js"
        self.python_code_file = None
        self.node_path = None
        
    def setup(self) -> None:
        try:
            result = subprocess.run(
                ["node", "--version"],
                capture_output=True,
                timeout=5
            )
            self.node_available = result.returncode == 0
        except (subprocess.TimeoutExpired, FileNotFoundError):
            self.node_available = False
        
        if not self.node_available:
            print("[WARN] Node.js not found, Pyodide executor will be skipped")
            return
        
        # Check if pyodide is installed (prioritize node_modules in benchmark directory)
        benchmark_dir = Path(__file__).parent
        local_node_modules = benchmark_dir / "node_modules"
        project_node_modules = PROJECT_ROOT / "node_modules"

        pyodide_found = False

        # Prioritize node_modules in benchmark directory
        if (local_node_modules / "pyodide").exists():
            self.node_path = str(local_node_modules)
            pyodide_found = True
        elif (project_node_modules / "pyodide").exists():
            self.node_path = str(project_node_modules)
            pyodide_found = True
        else:
            # Try global installation
            try:
                result = subprocess.run(
                    ["node", "-e", "require('pyodide')"],
                    capture_output=True,
                    timeout=10
                )
                if result.returncode == 0:
                    pyodide_found = True
            except (subprocess.TimeoutExpired, FileNotFoundError):
                pass
        
        if not pyodide_found:
            print("[WARN] Pyodide npm package not found.")
            print("       Install via: cd benchmark && npm install")
            return
        
        # Check if runner script exists
        if not self.pyodide_runner.exists():
            print("[WARN] Pyodide runner script not found")
            return

        # Write Python code to temporary file
        self.python_code_file = Path(tempfile.gettempdir()) / "pyodide_python_code.py"
        python_code = self.script_path.read_text()
        self.python_code_file.write_text(python_code)
        
        self.pyodide_available = True
    
    def teardown(self) -> None:
        if self.python_code_file and self.python_code_file.exists():
            self.python_code_file.unlink()
    
    def execute(self, input_json: str) -> BenchmarkResult:
        if not self.pyodide_available:
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=0,
                error="Pyodide not available"
            )
            
        start_time = time.perf_counter()
        try:
            env = os.environ.copy()
            env["PYTHON_CODE_PATH"] = str(self.python_code_file)
            
            # Set NODE_PATH so Node.js can find locally installed pyodide
            if self.node_path:
                env["NODE_PATH"] = self.node_path
            
            result = subprocess.run(
                ["node", str(self.pyodide_runner)],
                input=input_json,
                capture_output=True,
                text=True,
                timeout=60,
                env=env
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            return BenchmarkResult(
                executor_name=self.name,
                success=result.returncode == 0,
                latency_ms=latency_ms,
                stdout=result.stdout,
                stderr=result.stderr,
                error=None if result.returncode == 0 else f"Exit code: {result.returncode}"
            )
        except subprocess.TimeoutExpired:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout"
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e)
            )


def run_single_benchmark(executor: BaseExecutor, input_json: str) -> BenchmarkResult:
    """Run single benchmark"""
    return executor.execute(input_json)


def run_concurrent_benchmark(
    executor: BaseExecutor,
    input_json: str,
    num_requests: int,
    concurrency: int
) -> BenchmarkStats:
    """Run concurrent benchmark"""
    
    print(f"\n{'='*60}")
    print(f"Running: {executor.name}")
    print(f"Requests: {num_requests}, Concurrency: {concurrency}")
    print(f"{'='*60}")
    
    executor.setup()
    
    results: List[BenchmarkResult] = []
    start_time = time.perf_counter()
    
    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [
            pool.submit(run_single_benchmark, executor, input_json)
            for _ in range(num_requests)
        ]
        
        for i, future in enumerate(futures):
            try:
                result = future.result(timeout=60)
                results.append(result)
                
                if (i + 1) % max(1, num_requests // 10) == 0:
                    print(f"  Progress: {i + 1}/{num_requests} ({(i + 1) * 100 // num_requests}%)")
                    
            except Exception as e:
                results.append(BenchmarkResult(
                    executor_name=executor.name,
                    success=False,
                    latency_ms=0,
                    error=str(e)
                ))
    
    total_time = time.perf_counter() - start_time
    
    executor.teardown()
    
    successful = [r for r in results if r.success]
    failed = [r for r in results if not r.success]
    latencies = [r.latency_ms for r in successful]
    
    if not latencies:
        latencies = [0.0]
    
    stats = BenchmarkStats(
        executor_name=executor.name,
        total_requests=num_requests,
        successful_requests=len(successful),
        failed_requests=len(failed),
        min_latency_ms=min(latencies),
        max_latency_ms=max(latencies),
        avg_latency_ms=statistics.mean(latencies),
        p50_latency_ms=percentile(latencies, 50),
        p95_latency_ms=percentile(latencies, 95),
        p99_latency_ms=percentile(latencies, 99),
        throughput_rps=len(successful) / total_time if total_time > 0 else 0,
        total_time_sec=total_time
    )
    
    print(f"\nResults for {executor.name}:")
    print(f"  Success Rate: {len(successful)}/{num_requests} ({len(successful) * 100 // num_requests}%)")
    print(f"  Latency (ms): min={stats.min_latency_ms:.2f}, avg={stats.avg_latency_ms:.2f}, max={stats.max_latency_ms:.2f}")
    print(f"  Percentiles (ms): p50={stats.p50_latency_ms:.2f}, p95={stats.p95_latency_ms:.2f}, p99={stats.p99_latency_ms:.2f}")
    print(f"  Throughput: {stats.throughput_rps:.2f} req/s")
    print(f"  Total Time: {stats.total_time_sec:.2f}s")
    
    if failed:
        error_counts: Dict[str, int] = {}
        for r in failed:
            error = r.error or "Unknown"
            error_counts[error] = error_counts.get(error, 0) + 1
        print(f"  Errors: {error_counts}")
    
    return stats


def run_cold_start_benchmark(executor: BaseExecutor, input_json: str, iterations: int = 10) -> Dict:
    """Cold start test"""
    print(f"\n{'='*60}")
    print(f"Cold Start Test: {executor.name}")
    print(f"Iterations: {iterations}")
    print(f"{'='*60}")
    
    latencies = []
    
    for i in range(iterations):
        executor.setup()
        result = executor.execute(input_json)
        executor.teardown()
        
        if result.success:
            latencies.append(result.latency_ms)
            print(f"  Iteration {i + 1}: {result.latency_ms:.2f}ms")
        else:
            print(f"  Iteration {i + 1}: FAILED - {result.error}")
    
    if latencies:
        stats = {
            "executor": executor.name,
            "iterations": iterations,
            "successful": len(latencies),
            "min_ms": round(min(latencies), 2),
            "max_ms": round(max(latencies), 2),
            "avg_ms": round(statistics.mean(latencies), 2),
            "p50_ms": round(percentile(latencies, 50), 2),
            "p95_ms": round(percentile(latencies, 95), 2),
        }
        print(f"\nCold Start Summary:")
        print(f"  Avg: {stats['avg_ms']:.2f}ms, P50: {stats['p50_ms']:.2f}ms, P95: {stats['p95_ms']:.2f}ms")
        return stats
    
    return {"executor": executor.name, "error": "All iterations failed"}


def generate_test_input() -> str:
    """Generate test input"""
    return json.dumps({
        "operation": "multiply",
        "a": 123,
        "b": 456
    })


def print_comparison_table(all_stats: List[BenchmarkStats]) -> None:
    """Print comparison table"""
    print("\n" + "=" * 100)
    print("BENCHMARK COMPARISON SUMMARY")
    print("=" * 100)
    
    headers = ["Executor", "Success%", "Avg(ms)", "P50(ms)", "P95(ms)", "P99(ms)", "RPS"]
    widths = [35, 10, 10, 10, 10, 10, 10]
    
    header_line = " | ".join(h.ljust(w) for h, w in zip(headers, widths))
    print(header_line)
    print("-" * len(header_line))
    
    sorted_stats = sorted(all_stats, key=lambda s: s.avg_latency_ms)
    
    for stats in sorted_stats:
        success_rate = f"{stats.successful_requests * 100 // max(1, stats.total_requests)}%"
        row = [
            stats.executor_name[:35],
            success_rate,
            f"{stats.avg_latency_ms:.1f}",
            f"{stats.p50_latency_ms:.1f}",
            f"{stats.p95_latency_ms:.1f}",
            f"{stats.p99_latency_ms:.1f}",
            f"{stats.throughput_rps:.1f}",
        ]
        print(" | ".join(str(v).ljust(w) for v, w in zip(row, widths)))
    
    print("=" * 100)
    
    valid_stats = [s for s in sorted_stats if s.avg_latency_ms > 0]
    if len(valid_stats) >= 2:
        baseline = valid_stats[0]
        print(f"\nPerformance Analysis (baseline: {baseline.executor_name}):")
        for stats in valid_stats[1:]:
            ratio = stats.avg_latency_ms / baseline.avg_latency_ms if baseline.avg_latency_ms > 0 else 0
            print(f"  {stats.executor_name}: {ratio:.2f}x slower than baseline")


def main():
    """Main function"""
    import argparse
    
    parser = argparse.ArgumentParser(description="SkillBox Benchmark Runner")
    parser.add_argument("--requests", "-n", type=int, default=100, help="Number of requests")
    parser.add_argument("--concurrency", "-c", type=int, default=10, help="Concurrency level")
    parser.add_argument("--cold-start", action="store_true", help="Run cold start test")
    parser.add_argument("--cold-iterations", type=int, default=10, help="Cold start iterations")
    parser.add_argument("--skip-docker", action="store_true", help="Skip Docker tests")
    parser.add_argument("--skip-srt", action="store_true", help="Skip SRT tests")
    parser.add_argument("--skip-pyodide", action="store_true", help="Skip Pyodide tests")
    parser.add_argument("--output", "-o", type=str, help="Output JSON file")
    parser.add_argument("--sandbox-level", "-l", type=int, choices=[1, 2, 3], 
                        help="SkillBox sandbox level (1=no sandbox, 2=sandbox only, 3=sandbox+scan). "
                             "Can also be set via SKILLBOX_SANDBOX_LEVEL env var")
    parser.add_argument("--compare-levels", action="store_true",
                        help="Compare performance across all sandbox levels (1, 2, 3)")
    
    args = parser.parse_args()
    
    # Determine sandbox level
    sandbox_level = args.sandbox_level
    if sandbox_level is None:
        sandbox_level = int(os.environ.get("SKILLBOX_SANDBOX_LEVEL", "3"))
    
    print("=" * 60)
    print("SkillBox High-Concurrency Benchmark")
    print("=" * 60)
    print(f"Configuration:")
    print(f"  Requests: {args.requests}")
    print(f"  Concurrency: {args.concurrency}")
    print(f"  SkillBox Binary: {SKILLBOX_BIN}")
    print(f"  Test Skill: {CALCULATOR_SKILL}")
    
    if args.compare_levels:
        print(f"  Mode: Compare all sandbox levels (1, 2, 3)")
        # Test all security levels
        executors = [
            SkillBoxExecutor(sandbox_level=1),
            SkillBoxExecutor(sandbox_level=2),
            SkillBoxExecutor(sandbox_level=3),
        ]
    else:
        print(f"  Sandbox Level: {sandbox_level}")
        executors = [
            SkillBoxExecutor(sandbox_level=sandbox_level),
        ]
    
    if not args.skip_srt:
        executors.append(SRTExecutor())
    
    if not args.skip_pyodide:
        executors.append(PyodideExecutor())
    
    if not args.skip_docker:
        executors.append(DockerExecutor())
    
    input_json = generate_test_input()
    all_stats: List[BenchmarkStats] = []
    cold_start_results: List[Dict] = []
    
    if args.cold_start:
        print("\n" + "=" * 60)
        print("COLD START BENCHMARK")
        print("=" * 60)
        for executor in executors:
            result = run_cold_start_benchmark(executor, input_json, args.cold_iterations)
            cold_start_results.append(result)
    
    print("\n" + "=" * 60)
    print("HIGH CONCURRENCY BENCHMARK")
    print("=" * 60)
    
    for executor in executors:
        try:
            stats = run_concurrent_benchmark(
                executor,
                input_json,
                num_requests=args.requests,
                concurrency=args.concurrency
            )
            all_stats.append(stats)
        except Exception as e:
            print(f"[ERROR] {executor.name} failed: {e}")
    
    print_comparison_table(all_stats)
    
    if args.output:
        output_data = {
            "config": {
                "requests": args.requests,
                "concurrency": args.concurrency,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            },
            "concurrent_results": [s.to_dict() for s in all_stats],
            "cold_start_results": cold_start_results,
        }
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)
        print(f"\nResults saved to: {args.output}")


if __name__ == "__main__":
    main()
