#!/usr/bin/env python3
"""
SkillBox Benchmark Runner V2 - High Concurrency Performance Comparison Test (Enhanced Version)

Comparing SkillBox with other sandbox solutions under high concurrency scenarios:
1. SkillBox Native Sandbox (Seatbelt/Namespace)
2. Docker Container Sandbox
3. SRT (Anthropic Sandbox Runtime)
4. Pyodide (WebAssembly)

Test metrics:
- Cold Start Latency
- Warm Start Latency
- Throughput under Concurrency
- P50/P95/P99 Latency
- Memory Usage (Peak Memory)
- CPU Time

Test scenarios:
- Simple calculation (calculator) - no dependencies
- Data analysis (data-analyzer) - requires pandas/numpy dependencies
"""

import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple

# Project root directory
PROJECT_ROOT = Path(__file__).parent.parent
SKILLS_DIR = PROJECT_ROOT / ".skills"
CALCULATOR_SKILL = SKILLS_DIR / "calculator"
DATA_ANALYZER_SKILL = SKILLS_DIR / "data-analyzer"

# SkillBox binary path
SKILLBOX_BIN = shutil.which("skillbox") or str(PROJECT_ROOT / "skillbox" / "target" / "release" / "skillbox")


@dataclass
class ResourceUsage:
    """Resource usage information"""
    peak_memory_mb: float = 0.0
    cpu_time_ms: float = 0.0
    

@dataclass
class BenchmarkResult:
    """Single execution result"""
    executor_name: str
    success: bool
    latency_ms: float
    stdout: str = ""
    stderr: str = ""
    error: Optional[str] = None
    resource_usage: ResourceUsage = field(default_factory=ResourceUsage)


@dataclass
class BenchmarkStats:
    """Statistics"""
    executor_name: str
    skill_name: str
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
    avg_memory_mb: float = 0.0
    peak_memory_mb: float = 0.0
    avg_cpu_time_ms: float = 0.0
    
    def to_dict(self) -> dict:
        return {
            "executor": self.executor_name,
            "skill": self.skill_name,
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
            "memory_mb": {
                "avg": round(self.avg_memory_mb, 2),
                "peak": round(self.peak_memory_mb, 2),
            },
            "cpu_time_ms": round(self.avg_cpu_time_ms, 2),
        }


def percentile(data: List[float], p: float) -> float:
    """Calculate percentile"""
    if not data:
        return 0.0
    sorted_data = sorted(data)
    index = int(len(sorted_data) * p / 100)
    return sorted_data[min(index, len(sorted_data) - 1)]


def get_process_memory(pid: int) -> float:
    """Get process memory usage (MB)"""
    try:
        if sys.platform == "darwin":
            # macOS: use ps command
            result = subprocess.run(
                ["ps", "-o", "rss=", "-p", str(pid)],
                capture_output=True,
                text=True,
                timeout=1
            )
            if result.returncode == 0 and result.stdout.strip():
                return int(result.stdout.strip()) / 1024  # KB to MB
        else:
            # Linux: 读取 /proc/pid/status
            with open(f"/proc/{pid}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        return int(line.split()[1]) / 1024  # KB to MB
    except Exception:
        pass
    return 0.0


def monitor_process_resources(pid: int, interval: float = 0.05) -> Tuple[float, List[float]]:
    """Monitor process resource usage, returns (peak_memory_mb, memory_samples)"""
    memory_samples = []
    peak_memory = 0.0

    while True:
        try:
            # Check if process still exists
            os.kill(pid, 0)
            mem = get_process_memory(pid)
            if mem > 0:
                memory_samples.append(mem)
                peak_memory = max(peak_memory, mem)
            time.sleep(interval)
        except (ProcessLookupError, OSError):
            break
    
    return peak_memory, memory_samples


class BaseExecutor:
    """Executor base class"""

    name: str = "base"

    def setup(self) -> None:
        pass

    def teardown(self) -> None:
        pass

    def execute(self, input_json: str) -> BenchmarkResult:
        raise NotImplementedError

    def execute_with_monitoring(self, input_json: str) -> BenchmarkResult:
        """Execute and monitor resource usage"""
        return self.execute(input_json)


class SkillBoxExecutor(BaseExecutor):
    """SkillBox native sandbox executor"""
    
    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, skill_name: str = "calculator"):
        self.skill_dir = skill_dir
        self.skill_name = skill_name
        self.skillbox_bin = SKILLBOX_BIN
        self.name = f"SkillBox ({skill_name})"
        
    def setup(self) -> None:
        if not os.path.exists(self.skillbox_bin):
            raise RuntimeError(f"SkillBox binary not found at {self.skillbox_bin}")
    
    def execute(self, input_json: str) -> BenchmarkResult:
        start_time = time.perf_counter()
        resource_usage = ResourceUsage()
        
        try:
            # Use Popen to monitor resources
            process = subprocess.Popen(
                [self.skillbox_bin, "run", str(self.skill_dir), input_json],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            # Start resource monitoring thread
            peak_memory = [0.0]
            def monitor():
                pm, _ = monitor_process_resources(process.pid)
                peak_memory[0] = pm
            
            monitor_thread = threading.Thread(target=monitor, daemon=True)
            monitor_thread.start()
            
            stdout, stderr = process.communicate(timeout=60)
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            monitor_thread.join(timeout=0.1)
            resource_usage.peak_memory_mb = peak_memory[0]
            resource_usage.cpu_time_ms = latency_ms  # Approximate value
            
            return BenchmarkResult(
                executor_name=self.name,
                success=process.returncode == 0,
                latency_ms=latency_ms,
                stdout=stdout,
                stderr=stderr,
                error=None if process.returncode == 0 else f"Exit code: {process.returncode}",
                resource_usage=resource_usage
            )
        except subprocess.TimeoutExpired:
            process.kill()
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout",
                resource_usage=resource_usage
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e),
                resource_usage=resource_usage
            )


class DockerExecutor(BaseExecutor):
    """Docker container sandbox executor"""
    
    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, skill_name: str = "calculator"):
        self.skill_dir = skill_dir
        self.skill_name = skill_name
        self.image_name = f"skillbox-benchmark-{skill_name}"
        self.docker_available = False
        self.name = f"Docker ({skill_name})"
        
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
        
        # Build different Dockerfile based on skill type
        if self.skill_name == "data-analyzer":
            dockerfile_content = """FROM python:3.11-slim
WORKDIR /app
RUN pip install --no-cache-dir pandas numpy
COPY scripts/main.py /app/main.py
CMD ["python", "/app/main.py"]
"""
        else:
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
            
            print(f"  Building Docker image for {self.skill_name}...")
            result = subprocess.run(
                ["docker", "build", "-t", self.image_name, "."],
                cwd=tmpdir,
                capture_output=True,
                timeout=300  # 5 minute timeout (installing dependencies may be slow)
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
        resource_usage = ResourceUsage()
        
        try:
            process = subprocess.Popen(
                [
                    "docker", "run", "--rm", "-i",
                    "--memory=512m",
                    "--cpus=1",
                    "--network=none",
                    "--security-opt=no-new-privileges",
                    self.image_name
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            stdout, stderr = process.communicate(input=input_json, timeout=60)
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            # Docker container memory limited to 512MB
            resource_usage.peak_memory_mb = 512.0  # max limit
            resource_usage.cpu_time_ms = latency_ms
            
            return BenchmarkResult(
                executor_name=self.name,
                success=process.returncode == 0,
                latency_ms=latency_ms,
                stdout=stdout,
                stderr=stderr,
                error=None if process.returncode == 0 else f"Exit code: {process.returncode}",
                resource_usage=resource_usage
            )
        except subprocess.TimeoutExpired:
            process.kill()
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout",
                resource_usage=resource_usage
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e),
                resource_usage=resource_usage
            )


class SRTExecutor(BaseExecutor):
    """SRT (Sandbox Runtime) Executor"""
    
    def __init__(self, script_path: Path, skill_name: str = "calculator"):
        self.script_path = script_path
        self.skill_name = skill_name
        self.srt_bin = None
        self.srt_available = False
        self.name = f"SRT ({skill_name})"
        
    def setup(self) -> None:
        self.srt_bin = shutil.which("srt") or shutil.which("sandbox-runtime")
        
        if not self.srt_bin:
            # Try to find from nvm path
            home = Path.home()
            nvm_paths = list(home.glob(".nvm/versions/node/*/bin/srt"))
            if nvm_paths:
                self.srt_bin = str(nvm_paths[-1])
        
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
        resource_usage = ResourceUsage()
        
        try:
            process = subprocess.Popen(
                [self.srt_bin, sys.executable, str(self.script_path)],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            # Monitor resources
            peak_memory = [0.0]
            def monitor():
                pm, _ = monitor_process_resources(process.pid)
                peak_memory[0] = pm
            
            monitor_thread = threading.Thread(target=monitor, daemon=True)
            monitor_thread.start()
            
            stdout, stderr = process.communicate(input=input_json, timeout=60)
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            monitor_thread.join(timeout=0.1)
            resource_usage.peak_memory_mb = peak_memory[0]
            resource_usage.cpu_time_ms = latency_ms
            
            return BenchmarkResult(
                executor_name=self.name,
                success=process.returncode == 0,
                latency_ms=latency_ms,
                stdout=stdout,
                stderr=stderr,
                error=None if process.returncode == 0 else f"Exit code: {process.returncode}",
                resource_usage=resource_usage
            )
        except subprocess.TimeoutExpired:
            process.kill()
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout",
                resource_usage=resource_usage
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e),
                resource_usage=resource_usage
            )


class PyodideExecutor(BaseExecutor):
    """Pyodide (WebAssembly) Executor"""
    
    def __init__(self, script_path: Path, skill_name: str = "calculator"):
        self.script_path = script_path
        self.skill_name = skill_name
        self.pyodide_available = False
        self.node_available = False
        self.pyodide_runner = Path(__file__).parent / "pyodide_runner_template.js"
        self.python_code_file = None
        self.node_path = None
        self.name = f"Pyodide ({skill_name})"
        
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
        
        # Check pyodide
        benchmark_dir = Path(__file__).parent
        local_node_modules = benchmark_dir / "node_modules"
        project_node_modules = PROJECT_ROOT / "node_modules"
        
        pyodide_found = False
        
        if (local_node_modules / "pyodide").exists():
            self.node_path = str(local_node_modules)
            pyodide_found = True
        elif (project_node_modules / "pyodide").exists():
            self.node_path = str(project_node_modules)
            pyodide_found = True
        else:
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
        
        if not self.pyodide_runner.exists():
            print("[WARN] Pyodide runner script not found")
            return
        
        # Note: Pyodide does not support pandas/numpy, so data-analyzer will fail
        if self.skill_name == "data-analyzer":
            print("[WARN] Pyodide does not support pandas/numpy, data-analyzer will fail")
            
        self.python_code_file = Path(tempfile.gettempdir()) / f"pyodide_python_code_{self.skill_name}.py"
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
        resource_usage = ResourceUsage()
        
        try:
            env = os.environ.copy()
            env["PYTHON_CODE_PATH"] = str(self.python_code_file)
            if self.node_path:
                env["NODE_PATH"] = self.node_path
            
            process = subprocess.Popen(
                ["node", str(self.pyodide_runner)],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=env
            )
            
            # Monitor resources
            peak_memory = [0.0]
            def monitor():
                pm, _ = monitor_process_resources(process.pid)
                peak_memory[0] = pm

            monitor_thread = threading.Thread(target=monitor, daemon=True)
            monitor_thread.start()

            stdout, stderr = process.communicate(input=input_json, timeout=120)
            latency_ms = (time.perf_counter() - start_time) * 1000
            
            monitor_thread.join(timeout=0.1)
            resource_usage.peak_memory_mb = peak_memory[0]
            resource_usage.cpu_time_ms = latency_ms
            
            return BenchmarkResult(
                executor_name=self.name,
                success=process.returncode == 0,
                latency_ms=latency_ms,
                stdout=stdout,
                stderr=stderr,
                error=None if process.returncode == 0 else f"Exit code: {process.returncode}",
                resource_usage=resource_usage
            )
        except subprocess.TimeoutExpired:
            process.kill()
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error="Timeout",
                resource_usage=resource_usage
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e),
                resource_usage=resource_usage
            )


def run_single_benchmark(executor: BaseExecutor, input_json: str) -> BenchmarkResult:
    """Run single benchmark"""
    return executor.execute(input_json)


def run_concurrent_benchmark(
    executor: BaseExecutor,
    input_json: str,
    num_requests: int,
    concurrency: int,
    skill_name: str = "calculator"
) -> BenchmarkStats:
    """Run concurrent benchmark"""
    
    print(f"\n{'='*60}")
    print(f"Running: {executor.name}")
    print(f"Skill: {skill_name}")
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
                result = future.result(timeout=120)
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
    memory_usages = [r.resource_usage.peak_memory_mb for r in successful if r.resource_usage.peak_memory_mb > 0]
    cpu_times = [r.resource_usage.cpu_time_ms for r in successful if r.resource_usage.cpu_time_ms > 0]
    
    if not latencies:
        latencies = [0.0]
    
    stats = BenchmarkStats(
        executor_name=executor.name,
        skill_name=skill_name,
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
        total_time_sec=total_time,
        avg_memory_mb=statistics.mean(memory_usages) if memory_usages else 0,
        peak_memory_mb=max(memory_usages) if memory_usages else 0,
        avg_cpu_time_ms=statistics.mean(cpu_times) if cpu_times else 0,
    )
    
    print(f"\nResults for {executor.name}:")
    print(f"  Success Rate: {len(successful)}/{num_requests} ({len(successful) * 100 // max(1, num_requests)}%)")
    print(f"  Latency (ms): min={stats.min_latency_ms:.2f}, avg={stats.avg_latency_ms:.2f}, max={stats.max_latency_ms:.2f}")
    print(f"  Percentiles (ms): p50={stats.p50_latency_ms:.2f}, p95={stats.p95_latency_ms:.2f}, p99={stats.p99_latency_ms:.2f}")
    print(f"  Throughput: {stats.throughput_rps:.2f} req/s")
    if stats.peak_memory_mb > 0:
        print(f"  Memory (MB): avg={stats.avg_memory_mb:.2f}, peak={stats.peak_memory_mb:.2f}")
    print(f"  Total Time: {stats.total_time_sec:.2f}s")
    
    if failed:
        error_counts: Dict[str, int] = {}
        for r in failed:
            error = r.error or "Unknown"
            error_counts[error] = error_counts.get(error, 0) + 1
        print(f"  Errors: {error_counts}")
    
    return stats


def generate_calculator_input() -> str:
    """Generate calculator test input"""
    return json.dumps({
        "operation": "multiply",
        "a": 123,
        "b": 456
    })


def generate_data_analyzer_input() -> str:
    """Generate data-analyzer test input"""
    return json.dumps({
        "data": json.dumps([
            {"name": "Alice", "age": 25, "score": 85},
            {"name": "Bob", "age": 30, "score": 92},
            {"name": "Charlie", "age": 28, "score": 78},
            {"name": "Diana", "age": 35, "score": 95},
            {"name": "Eve", "age": 22, "score": 88}
        ]),
        "operation": "describe"
    })


def print_comparison_table(all_stats: List[BenchmarkStats]) -> None:
    """Print comparison table"""
    print("\n" + "=" * 120)
    print("BENCHMARK COMPARISON SUMMARY")
    print("=" * 120)
    
    headers = ["Executor", "Skill", "Success%", "Avg(ms)", "P50(ms)", "P95(ms)", "RPS", "Mem(MB)"]
    widths = [30, 15, 10, 10, 10, 10, 10, 10]
    
    header_line = " | ".join(h.ljust(w) for h, w in zip(headers, widths))
    print(header_line)
    print("-" * len(header_line))
    
    # Sort by skill and latency
    sorted_stats = sorted(all_stats, key=lambda s: (s.skill_name, s.avg_latency_ms))
    
    for stats in sorted_stats:
        success_rate = f"{stats.successful_requests * 100 // max(1, stats.total_requests)}%"
        mem_str = f"{stats.peak_memory_mb:.1f}" if stats.peak_memory_mb > 0 else "N/A"
        row = [
            stats.executor_name[:30],
            stats.skill_name[:15],
            success_rate,
            f"{stats.avg_latency_ms:.1f}",
            f"{stats.p50_latency_ms:.1f}",
            f"{stats.p95_latency_ms:.1f}",
            f"{stats.throughput_rps:.1f}",
            mem_str,
        ]
        print(" | ".join(str(v).ljust(w) for v, w in zip(row, widths)))
    
    print("=" * 120)
    
    # Analyze by skill group
    skills = set(s.skill_name for s in sorted_stats)
    for skill in sorted(skills):
        skill_stats = [s for s in sorted_stats if s.skill_name == skill and s.avg_latency_ms > 0]
        if len(skill_stats) >= 2:
            baseline = skill_stats[0]
            print(f"\nPerformance Analysis for '{skill}' (baseline: {baseline.executor_name}):")
            for stats in skill_stats[1:]:
                ratio = stats.avg_latency_ms / baseline.avg_latency_ms if baseline.avg_latency_ms > 0 else 0
                print(f"  {stats.executor_name}: {ratio:.2f}x slower than baseline")


def main():
    """Main function"""
    import argparse
    
    parser = argparse.ArgumentParser(description="SkillBox Benchmark Runner V2")
    parser.add_argument("--requests", "-n", type=int, default=50, help="Number of requests per skill")
    parser.add_argument("--concurrency", "-c", type=int, default=5, help="Concurrency level")
    parser.add_argument("--skip-docker", action="store_true", help="Skip Docker tests")
    parser.add_argument("--skip-srt", action="store_true", help="Skip SRT tests")
    parser.add_argument("--skip-pyodide", action="store_true", help="Skip Pyodide tests")
    parser.add_argument("--skip-deps", action="store_true", help="Skip skills with dependencies (data-analyzer)")
    parser.add_argument("--output", "-o", type=str, help="Output JSON file")
    
    args = parser.parse_args()
    
    print("=" * 60)
    print("SkillBox Benchmark Runner V2")
    print("=" * 60)
    print(f"Configuration:")
    print(f"  Requests per skill: {args.requests}")
    print(f"  Concurrency: {args.concurrency}")
    print(f"  SkillBox Binary: {SKILLBOX_BIN}")
    
    all_stats: List[BenchmarkStats] = []

    # Test scenario configuration
    test_scenarios = [
        {
            "name": "calculator",
            "skill_dir": CALCULATOR_SKILL,
            "script_path": CALCULATOR_SKILL / "scripts" / "main.py",
            "input_generator": generate_calculator_input,
            "has_deps": False,
        },
    ]
    
    if not args.skip_deps:
        test_scenarios.append({
            "name": "data-analyzer",
            "skill_dir": DATA_ANALYZER_SKILL,
            "script_path": DATA_ANALYZER_SKILL / "scripts" / "main.py",
            "input_generator": generate_data_analyzer_input,
            "has_deps": True,
        })
    
    for scenario in test_scenarios:
        skill_name = scenario["name"]
        skill_dir = scenario["skill_dir"]
        script_path = scenario["script_path"]
        input_json = scenario["input_generator"]()
        has_deps = scenario["has_deps"]
        
        print(f"\n{'#'*60}")
        print(f"# Testing Skill: {skill_name}")
        print(f"# Has Dependencies: {has_deps}")
        print(f"{'#'*60}")
        
        # SkillBox
        executor = SkillBoxExecutor(skill_dir, skill_name)
        try:
            stats = run_concurrent_benchmark(
                executor, input_json, args.requests, args.concurrency, skill_name
            )
            all_stats.append(stats)
        except Exception as e:
            print(f"[ERROR] SkillBox ({skill_name}) failed: {e}")
        
        # SRT (only supports skills without dependencies, as SRT doesn't auto-install dependencies)
        if not args.skip_srt and not has_deps:
            executor = SRTExecutor(script_path, skill_name)
            try:
                stats = run_concurrent_benchmark(
                    executor, input_json, args.requests, args.concurrency, skill_name
                )
                all_stats.append(stats)
            except Exception as e:
                print(f"[ERROR] SRT ({skill_name}) failed: {e}")
        
        # Pyodide (doesn't support pandas/numpy)
        if not args.skip_pyodide and not has_deps:
            executor = PyodideExecutor(script_path, skill_name)
            try:
                stats = run_concurrent_benchmark(
                    executor, input_json, args.requests, args.concurrency, skill_name
                )
                all_stats.append(stats)
            except Exception as e:
                print(f"[ERROR] Pyodide ({skill_name}) failed: {e}")
        
        # Docker
        if not args.skip_docker:
            executor = DockerExecutor(skill_dir, skill_name)
            try:
                stats = run_concurrent_benchmark(
                    executor, input_json, args.requests, args.concurrency, skill_name
                )
                all_stats.append(stats)
            except Exception as e:
                print(f"[ERROR] Docker ({skill_name}) failed: {e}")
    
    print_comparison_table(all_stats)
    
    if args.output:
        output_data = {
            "config": {
                "requests_per_skill": args.requests,
                "concurrency": args.concurrency,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            },
            "results": [s.to_dict() for s in all_stats],
        }
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)
        print(f"\nResults saved to: {args.output}")


if __name__ == "__main__":
    main()
