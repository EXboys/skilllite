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

Note: gVisor runs ON TOP OF Docker (--runtime=runsc), so its performance will
always be worse than Docker. It's only useful for security isolation comparison,
not performance benchmarking. See security_vs.py for security comparison tests.

Test metrics:
- Cold Start Latency
- Warm Start Latency
- Throughput under Concurrency
- P50/P95/P99 Latency
- Resource Usage (CPU/Memory)
"""

import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
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
SKILLBOX_BIN = shutil.which("skilllite") or str(PROJECT_ROOT / "skilllite" / "target" / "release" / "skilllite")

# Add python-sdk for IPC executor (uses skilllite serve --stdio daemon)
sys.path.insert(0, str(PROJECT_ROOT / "python-sdk"))

# Load .env if available (SKILLBOX_QUIET, etc.)
try:
    from dotenv import load_dotenv
    load_dotenv(PROJECT_ROOT / ".env")
except ImportError:
    pass


@dataclass
class BenchmarkResult:
    """Single execution result"""
    executor_name: str
    success: bool
    latency_ms: float
    stdout: str = ""
    stderr: str = ""
    error: Optional[str] = None
    memory_kb: float = 0.0  # Peak memory usage in KB


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
    avg_memory_mb: float = 0.0  # Average memory usage in MB
    peak_memory_mb: float = 0.0  # Peak memory usage in MB
    
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
            "memory_mb": {
                "avg": round(self.avg_memory_mb, 2),
                "peak": round(self.peak_memory_mb, 2),
            },
        }


def percentile(data: List[float], p: float) -> float:
    """Calculate percentile"""
    if not data:
        return 0.0
    sorted_data = sorted(data)
    index = int(len(sorted_data) * p / 100)
    return sorted_data[min(index, len(sorted_data) - 1)]


class ResourceMonitor:
    """Resource monitor - measures process memory consumption"""

    @staticmethod
    def get_peak_memory_kb(command: list, cwd: str = None, timeout: int = 30, input_data: str = None, env: dict = None) -> tuple:
        """
        Run command and get peak memory usage
        Returns: (elapsed_ms, success, stdout, stderr, peak_memory_kb)
        """
        is_macos = platform.system() == "Darwin"

        if is_macos:
            # macOS: use /usr/bin/time -l
            full_command = ["/usr/bin/time", "-l"] + command
            start = time.perf_counter()
            try:
                # Merge with current environment if env is provided
                run_env = os.environ.copy()
                if env:
                    run_env.update(env)
                
                result = subprocess.run(
                    full_command,
                    capture_output=True,
                    timeout=timeout,
                    cwd=cwd,
                    input=input_data.encode() if input_data else None,
                    text=False if input_data else True,
                    env=run_env if env else None
                )
                end = time.perf_counter()
                elapsed_ms = (end - start) * 1000
                
                stderr_text = result.stderr.decode(errors='replace') if isinstance(result.stderr, bytes) else result.stderr
                # macOS time output format: "maximum resident set size" in bytes
                memory_kb = 0
                for line in stderr_text.split('\n'):
                    if 'maximum resident set size' in line.lower():
                        try:
                            # Extract number (bytes)
                            parts = line.strip().split()
                            memory_bytes = int(parts[0])
                            memory_kb = memory_bytes / 1024
                        except (ValueError, IndexError):
                            pass
                        break
                
                stdout_text = result.stdout.decode(errors='replace') if isinstance(result.stdout, bytes) else result.stdout
                return (
                    elapsed_ms,
                    result.returncode == 0,
                    stdout_text,
                    stderr_text,
                    memory_kb
                )
            except subprocess.TimeoutExpired:
                return (timeout * 1000, False, "", "Timeout", 0)
            except Exception as e:
                return (0, False, "", str(e), 0)
        else:
            # Linux: use /usr/bin/time -v
            full_command = ["/usr/bin/time", "-v"] + command
            start = time.perf_counter()
            try:
                # Merge with current environment if env is provided
                run_env = os.environ.copy()
                if env:
                    run_env.update(env)
                
                result = subprocess.run(
                    full_command,
                    capture_output=True,
                    timeout=timeout,
                    cwd=cwd,
                    input=input_data.encode() if input_data else None,
                    text=False if input_data else True,
                    env=run_env if env else None
                )
                end = time.perf_counter()
                elapsed_ms = (end - start) * 1000
                
                stderr_text = result.stderr.decode(errors='replace') if isinstance(result.stderr, bytes) else result.stderr
                # Linux time output format: "Maximum resident set size (kbytes):"
                memory_kb = 0
                for line in stderr_text.split('\n'):
                    if 'maximum resident set size' in line.lower():
                        try:
                            parts = line.strip().split(':')
                            memory_kb = float(parts[-1].strip())
                        except (ValueError, IndexError):
                            pass
                        break
                
                stdout_text = result.stdout.decode(errors='replace') if isinstance(result.stdout, bytes) else result.stdout
                return (
                    elapsed_ms,
                    result.returncode == 0,
                    stdout_text,
                    stderr_text,
                    memory_kb
                )
            except subprocess.TimeoutExpired:
                return (timeout * 1000, False, "", "Timeout", 0)
            except Exception as e:
                return (0, False, "", str(e), 0)


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

    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, sandbox_level: Optional[int] = None, measure_memory: bool = True):
        self.skill_dir = skill_dir
        self.skilllite_bin = SKILLBOX_BIN
        self.measure_memory = measure_memory
        self.resource_monitor = ResourceMonitor() if measure_memory else None
        # Get sandbox level from environment variable or parameter, default is 3
        if sandbox_level is not None:
            self.sandbox_level = sandbox_level
        else:
            self.sandbox_level = int(os.environ.get("SKILLBOX_SANDBOX_LEVEL", "3"))
        # Update executor name to reflect security level
        self.name = f"SkillBox (Level {self.sandbox_level})"
        
    def setup(self) -> None:
        if not os.path.exists(self.skilllite_bin):
            raise RuntimeError(f"SkillBox binary not found at {self.skilllite_bin}")
    
    def execute(self, input_json: str) -> BenchmarkResult:
        if self.measure_memory and self.resource_monitor:
            # Measure memory usage
            try:
                skilllite_env = {
                    "SKILLBOX_QUIET": "1",
                    "SKILLBOX_SKILLS_ROOT": str(PROJECT_ROOT),
                }
                elapsed_ms, success, stdout, stderr, memory_kb = self.resource_monitor.get_peak_memory_kb(
                    [self.skilllite_bin, "run", "--sandbox-level", str(self.sandbox_level), str(self.skill_dir), input_json],
                    timeout=30,
                    env=skilllite_env
                )
                return BenchmarkResult(
                    executor_name=self.name,
                    success=success,
                    latency_ms=elapsed_ms,
                    stdout=stdout,
                    stderr=stderr,
                    error=None if success else f"Exit code: {1 if not success else 0}",
                    memory_kb=memory_kb
                )
            except Exception as e:
                return BenchmarkResult(
                    executor_name=self.name,
                    success=False,
                    latency_ms=0,
                    error=str(e),
                    memory_kb=0
                )
        else:
            # Original implementation without memory measurement
            start_time = time.perf_counter()
            try:
                # Set environment variable to pass sandbox level and skills root
                env = os.environ.copy()
                env["SKILLBOX_SANDBOX_LEVEL"] = str(self.sandbox_level)
                env["SKILLBOX_QUIET"] = "1"  # Suppress [INFO] to avoid perf impact
                env["SKILLBOX_SKILLS_ROOT"] = str(PROJECT_ROOT)  # Allow .skills under project root
                
                # Use --sandbox-level CLI argument (takes precedence over env var)
                result = subprocess.run(
                    [self.skilllite_bin, "run", "--sandbox-level", str(self.sandbox_level), str(self.skill_dir), input_json],
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


class SkillBoxIPCExecutor(BaseExecutor):
    """
    SkillBox via IPC (skilllite serve --stdio daemon).
    Uses ipc.IPCClient to avoid cold-start overhead per request.
    """

    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, sandbox_level: Optional[int] = None, measure_memory: bool = False):
        self.skill_dir = skill_dir
        self.sandbox_level = sandbox_level or int(os.environ.get("SKILLBOX_SANDBOX_LEVEL", "3"))
        self.name = f"SkillBox IPC (Level {self.sandbox_level})"
        self._client = None
        self.measure_memory = measure_memory

    def setup(self) -> None:
        os.environ["SKILLBOX_USE_IPC"] = "1"
        os.environ["SKILLBOX_QUIET"] = "1"
        # IPC daemon validates skill path against SKILLBOX_SKILLS_ROOT; must be set before client starts
        os.environ["SKILLBOX_SKILLS_ROOT"] = str(PROJECT_ROOT)
        from skilllite.ipc import _get_client
        self._client = _get_client()
        if not self._client:
            raise RuntimeError(
                "IPC client not available. Ensure skilllite binary is installed: pip install skilllite"
            )

    def teardown(self) -> None:
        from skilllite.ipc import _shutdown_client
        _shutdown_client()

    def execute(self, input_json: str) -> BenchmarkResult:
        if not self._client:
            return BenchmarkResult(
                executor_name=self.name, success=False, latency_ms=0,
                error="IPC client not initialized"
            )
        start_time = time.perf_counter()
        try:
            res = self._client.run(
                str(self.skill_dir),
                input_json,
                sandbox_level=self.sandbox_level,
            )
            latency_ms = (time.perf_counter() - start_time) * 1000
            output = res.get("output", "")
            exit_code = res.get("exit_code", 0)
            return BenchmarkResult(
                executor_name=self.name,
                success=exit_code == 0,
                latency_ms=latency_ms,
                stdout=output,
                stderr="",
                error=None if exit_code == 0 else f"Exit code: {exit_code}",
            )
        except Exception as e:
            latency_ms = (time.perf_counter() - start_time) * 1000
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=latency_ms,
                error=str(e),
            )


class DockerExecutor(BaseExecutor):
    """Docker container sandbox executor"""
    
    name = "Docker Container"
    
    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, measure_memory: bool = False):
        self.skill_dir = skill_dir
        self.image_name = "skilllite-benchmark-python"
        self.docker_available = False
        self.measure_memory = measure_memory
        self.resource_monitor = ResourceMonitor() if measure_memory else None
        
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
        
        if self.measure_memory and self.resource_monitor:
            # Measure Docker container memory usage accurately using docker stats
            # Use a background monitoring approach to capture peak memory
            import uuid
            container_name = None
            peak_memory_kb = [0]  # Use list to allow modification in nested function
            
            try:
                container_name = f"benchmark-{uuid.uuid4().hex[:8]}"
                start_time = time.perf_counter()
                
                # Start container in detached mode; override CMD with sleep so container stays
                # running (default CMD runs main.py which needs stdin and would exit immediately)
                create_result = subprocess.run(
                    [
                        "docker", "run", "-d", "--name", container_name,
                        "--memory=1g",  # 1GB to avoid OOM (Exit code 137)
                        "--cpus=1",
                        "--network=none",
                        "--security-opt=no-new-privileges",
                        self.image_name,
                        "sleep", "infinity"
                    ],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                
                if create_result.returncode != 0:
                    raise Exception(f"Failed to create container: {create_result.stderr}")
                
                # Monitor memory in background
                stop_monitoring = threading.Event()
                
                def monitor_memory():
                    while not stop_monitoring.is_set():
                        try:
                            stats_result = subprocess.run(
                                ["docker", "stats", "--no-stream", "--format", "{{.MemUsage}}", container_name],
                                capture_output=True,
                                text=True,
                                timeout=2
                            )
                            if stats_result.returncode == 0 and stats_result.stdout.strip():
                                mem_str = stats_result.stdout.strip().split()[0]
                                # Parse memory value
                                if "MiB" in mem_str or "MB" in mem_str:
                                    mem_value = float(mem_str.replace("MiB", "").replace("MB", ""))
                                    current_kb = mem_value * 1024
                                elif "KiB" in mem_str or "KB" in mem_str:
                                    mem_value = float(mem_str.replace("KiB", "").replace("KB", ""))
                                    current_kb = mem_value
                                elif "GiB" in mem_str or "GB" in mem_str:
                                    mem_value = float(mem_str.replace("GiB", "").replace("GB", ""))
                                    current_kb = mem_value * 1024 * 1024
                                else:
                                    current_kb = 0
                                
                                if current_kb > peak_memory_kb[0]:
                                    peak_memory_kb[0] = current_kb
                        except:
                            pass
                        if not stop_monitoring.wait(0.1):  # Check every 100ms or stop if signaled
                            continue
                        break
                
                monitor_thread = threading.Thread(target=monitor_memory, daemon=True)
                monitor_thread.start()
                
                # Send input to container and execute
                exec_result = subprocess.run(
                    ["docker", "exec", "-i", container_name, "python", "/app/main.py"],
                    input=input_json,
                    capture_output=True,
                    text=True,
                    timeout=30
                )
                
                # Stop monitoring and wait for final capture
                stop_monitoring.set()
                time.sleep(0.2)
                monitor_thread.join(timeout=0.5)
                
                elapsed_ms = (time.perf_counter() - start_time) * 1000
                
                # Use peak memory or fallback estimate
                memory_kb = peak_memory_kb[0] if peak_memory_kb[0] > 0 else 150 * 1024
                
                # Clean up container
                subprocess.run(["docker", "rm", "-f", container_name], capture_output=True, timeout=5)
                
                return BenchmarkResult(
                    executor_name=self.name,
                    success=exec_result.returncode == 0,
                    latency_ms=elapsed_ms,
                    stdout=exec_result.stdout,
                    stderr=exec_result.stderr,
                    error=None if exec_result.returncode == 0 else f"Exit code: {exec_result.returncode}",
                    memory_kb=memory_kb
                )
            except Exception as e:
                # Clean up container on error
                if container_name:
                    try:
                        subprocess.run(["docker", "rm", "-f", container_name], capture_output=True, timeout=2)
                    except:
                        pass
                return BenchmarkResult(
                    executor_name=self.name,
                    success=False,
                    latency_ms=0,
                    error=str(e),
                    memory_kb=0
                )
        else:
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


class GvisorExecutor(BaseExecutor):
    """gVisor sandbox executor using Docker with runsc runtime
    
    DEPRECATED: gVisor runs ON TOP OF Docker (--runtime=runsc), so its performance
    will always be worse than Docker. It's only useful for security isolation
    comparison, not performance benchmarking.
    
    If you need gVisor comparison, use security_vs.py instead of benchmark_runner.py.
    
    Installation:
    - Linux: sudo apt-get install runsc (or download from https://gvisor.dev)
    - Configure Docker: sudo runsc install && sudo systemctl restart docker
    """
    
    name = "gVisor (runsc)"
    
    def __init__(self, skill_dir: Path = CALCULATOR_SKILL, measure_memory: bool = False):
        self.skill_dir = skill_dir
        self.image_name = "skilllite-benchmark-python"
        self.gvisor_available = False
        self.docker_available = False
        self.setup_error = None
        self.measure_memory = measure_memory
        self.resource_monitor = ResourceMonitor() if measure_memory else None
        
    def setup(self) -> None:
        self.setup_error = None  # Store error message for later reporting
        
        # Check platform - gVisor only supports Linux
        if platform.system() != "Linux":
            self.setup_error = f"gVisor only supports Linux (current: {platform.system()}). Use Linux or skip gVisor test."
            print(f"[WARN] {self.setup_error}")
            return
        
        # Check Docker availability
        try:
            result = subprocess.run(
                ["docker", "version"],
                capture_output=True,
                timeout=5
            )
            self.docker_available = result.returncode == 0
            if not self.docker_available:
                self.setup_error = "Docker not available"
                print(f"[WARN] Docker not available, {self.name} will be skipped")
                return
        except (subprocess.TimeoutExpired, FileNotFoundError) as e:
            self.docker_available = False
            self.setup_error = f"Docker not found: {e}"
            print(f"[WARN] Docker not available, {self.name} will be skipped")
            return
        
        # Check if runsc runtime is available
        runsc_found = False
        try:
            result = subprocess.run(
                ["runsc", "--version"],
                capture_output=True,
                timeout=5
            )
            if result.returncode == 0:
                runsc_found = True
        except (subprocess.TimeoutExpired, FileNotFoundError):
            pass
        
        # If runsc command not found, check if it's configured in Docker
        if not runsc_found:
            try:
                result = subprocess.run(
                    ["docker", "info", "--format", "{{.Runtimes}}"],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if result.returncode == 0 and "runsc" in result.stdout:
                    runsc_found = True
            except:
                pass
        
        if not runsc_found:
            self.setup_error = "gVisor (runsc) runtime not found. Install via: sudo apt-get install runsc && sudo runsc install && sudo systemctl restart docker"
            print(f"[WARN] {self.setup_error}")
            return
        
        self.gvisor_available = True
        
        # Build Docker image if needed (reuse same image as DockerExecutor)
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
                self.gvisor_available = False
    
    def teardown(self) -> None:
        # Image cleanup is handled by DockerExecutor, so we don't need to remove it here
        pass
    
    def execute(self, input_json: str) -> BenchmarkResult:
        if not self.gvisor_available or not self.docker_available:
            error_msg = self.setup_error or "gVisor not available"
            return BenchmarkResult(
                executor_name=self.name,
                success=False,
                latency_ms=0,
                error=error_msg
            )
        
        if self.measure_memory and self.resource_monitor:
            # Measure gVisor container memory usage using docker stats
            import uuid
            container_name = None
            peak_memory_kb = [0]
            
            try:
                container_name = f"benchmark-gvisor-{uuid.uuid4().hex[:8]}"
                start_time = time.perf_counter()
                
                # Start container with gVisor runtime in detached mode; override CMD so it stays running
                create_result = subprocess.run(
                    [
                        "docker", "run", "-d", "--name", container_name,
                        "--runtime=runsc",
                        "--memory=1g",
                        "--cpus=1",
                        "--network=none",
                        "--security-opt=no-new-privileges",
                        self.image_name,
                        "sleep", "infinity"
                    ],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                
                if create_result.returncode != 0:
                    raise Exception(f"Failed to create gVisor container: {create_result.stderr}")
                
                # Monitor memory in background
                stop_monitoring = threading.Event()
                
                def monitor_memory():
                    while not stop_monitoring.is_set():
                        try:
                            stats_result = subprocess.run(
                                ["docker", "stats", "--no-stream", "--format", "{{.MemUsage}}", container_name],
                                capture_output=True,
                                text=True,
                                timeout=2
                            )
                            if stats_result.returncode == 0 and stats_result.stdout.strip():
                                mem_str = stats_result.stdout.strip().split()[0]
                                if "MiB" in mem_str or "MB" in mem_str:
                                    mem_value = float(mem_str.replace("MiB", "").replace("MB", ""))
                                    current_kb = mem_value * 1024
                                elif "KiB" in mem_str or "KB" in mem_str:
                                    mem_value = float(mem_str.replace("KiB", "").replace("KB", ""))
                                    current_kb = mem_value
                                elif "GiB" in mem_str or "GB" in mem_str:
                                    mem_value = float(mem_str.replace("GiB", "").replace("GB", ""))
                                    current_kb = mem_value * 1024 * 1024
                                else:
                                    current_kb = 0
                                
                                if current_kb > peak_memory_kb[0]:
                                    peak_memory_kb[0] = current_kb
                        except:
                            pass
                        if not stop_monitoring.wait(0.1):
                            continue
                        break
                
                monitor_thread = threading.Thread(target=monitor_memory, daemon=True)
                monitor_thread.start()
                
                # Send input to container and execute
                exec_result = subprocess.run(
                    ["docker", "exec", "-i", container_name, "python", "/app/main.py"],
                    input=input_json,
                    capture_output=True,
                    text=True,
                    timeout=30
                )
                
                # Stop monitoring
                stop_monitoring.set()
                time.sleep(0.2)
                monitor_thread.join(timeout=0.5)
                
                elapsed_ms = (time.perf_counter() - start_time) * 1000
                memory_kb = peak_memory_kb[0] if peak_memory_kb[0] > 0 else 150 * 1024
                
                # Clean up container
                subprocess.run(["docker", "rm", "-f", container_name], capture_output=True, timeout=5)
                
                return BenchmarkResult(
                    executor_name=self.name,
                    success=exec_result.returncode == 0,
                    latency_ms=elapsed_ms,
                    stdout=exec_result.stdout,
                    stderr=exec_result.stderr,
                    error=None if exec_result.returncode == 0 else f"Exit code: {exec_result.returncode}",
                    memory_kb=memory_kb
                )
            except Exception as e:
                if container_name:
                    try:
                        subprocess.run(["docker", "rm", "-f", container_name], capture_output=True, timeout=2)
                    except:
                        pass
                return BenchmarkResult(
                    executor_name=self.name,
                    success=False,
                    latency_ms=0,
                    error=str(e),
                    memory_kb=0
                )
        else:
            start_time = time.perf_counter()
            try:
                # Use gVisor runtime with Docker
                result = subprocess.run(
                    [
                        "docker", "run", "--rm", "-i",
                        "--runtime=runsc",
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
    
    def __init__(self, script_path: Path = CALCULATOR_SKILL / "scripts" / "main.py", measure_memory: bool = False):
        self.script_path = script_path
        self.srt_bin = None
        self.srt_available = False
        self.measure_memory = measure_memory
        self.resource_monitor = ResourceMonitor() if measure_memory else None
        
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
        
        if self.measure_memory and self.resource_monitor:
            # Measure memory usage
            try:
                elapsed_ms, success, stdout, stderr, memory_kb = self.resource_monitor.get_peak_memory_kb(
                    [self.srt_bin, sys.executable, str(self.script_path)],
                    timeout=30,
                    input_data=input_json
                )
                return BenchmarkResult(
                    executor_name=self.name,
                    success=success,
                    latency_ms=elapsed_ms,
                    stdout=stdout,
                    stderr=stderr,
                    error=None if success else f"Exit code: {1 if not success else 0}",
                    memory_kb=memory_kb
                )
            except Exception as e:
                return BenchmarkResult(
                    executor_name=self.name,
                    success=False,
                    latency_ms=0,
                    error=str(e),
                    memory_kb=0
                )
        else:
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
    
    def __init__(self, script_path: Path = CALCULATOR_SKILL / "scripts" / "main.py", measure_memory: bool = False):
        self.script_path = script_path
        self.pyodide_available = False
        self.node_available = False
        self.pyodide_runner = Path(__file__).parent / "pyodide_runner_template.js"
        self.python_code_file = None
        self.node_path = None
        self.measure_memory = measure_memory
        self.resource_monitor = ResourceMonitor() if measure_memory else None
        
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
        self.python_code_file = Path(tempfile.gettempdir()) / f"pyodide_python_code_{os.getpid()}.py"
        python_code = self.script_path.read_text()
        self.python_code_file.write_text(python_code)
        
        # Verify file exists and is readable
        if not self.python_code_file.exists():
            print(f"[WARN] Failed to create Python code file: {self.python_code_file}")
            return
        
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
        
        if self.measure_memory and self.resource_monitor:
            # Measure memory usage
            try:
                # Verify Python code file exists
                if not self.python_code_file or not self.python_code_file.exists():
                    raise Exception(f"Python code file not found: {self.python_code_file}")
                
                env = os.environ.copy()
                env["PYTHON_CODE_PATH"] = str(self.python_code_file.absolute())
                if self.node_path:
                    env["NODE_PATH"] = self.node_path
                
                elapsed_ms, success, stdout, stderr, memory_kb = self.resource_monitor.get_peak_memory_kb(
                    ["node", str(self.pyodide_runner.absolute())],
                    timeout=60,
                    input_data=input_json,
                    cwd=str(self.pyodide_runner.parent.absolute()) if self.pyodide_runner.parent else None,
                    env=env
                )
                
                # Check if execution actually succeeded
                if not success or (stdout and "Pyodide error" in stdout):
                    # Try to extract error message
                    error_msg = stderr if stderr else stdout
                    if "Pyodide error" in stdout:
                        error_msg = stdout.split("Pyodide error:")[-1].strip()
                    return BenchmarkResult(
                        executor_name=self.name,
                        success=False,
                        latency_ms=elapsed_ms,
                        stdout=stdout,
                        stderr=stderr,
                        error=error_msg[:200] if error_msg else "Pyodide execution failed",
                        memory_kb=memory_kb
                    )
                
                return BenchmarkResult(
                    executor_name=self.name,
                    success=success,
                    latency_ms=elapsed_ms,
                    stdout=stdout,
                    stderr=stderr,
                    error=None if success else f"Exit code: {1 if not success else 0}",
                    memory_kb=memory_kb
                )
            except Exception as e:
                return BenchmarkResult(
                    executor_name=self.name,
                    success=False,
                    latency_ms=0,
                    error=str(e),
                    memory_kb=0
                )
        else:
            start_time = time.perf_counter()
            try:
                # Verify Python code file exists
                if not self.python_code_file or not self.python_code_file.exists():
                    raise Exception(f"Python code file not found: {self.python_code_file}")
                
                env = os.environ.copy()
                env["PYTHON_CODE_PATH"] = str(self.python_code_file.absolute())
                
                # Set NODE_PATH so Node.js can find locally installed pyodide
                if self.node_path:
                    env["NODE_PATH"] = self.node_path
                
                # Use absolute path for runner script
                runner_path = str(self.pyodide_runner.absolute())
                
                result = subprocess.run(
                    ["node", runner_path],
                    input=input_json,
                    capture_output=True,
                    text=True,
                    timeout=60,
                    env=env,
                    cwd=str(self.pyodide_runner.parent.absolute()) if self.pyodide_runner.parent else None
                )
                latency_ms = (time.perf_counter() - start_time) * 1000
                
                # Check for Pyodide errors in output
                if result.returncode != 0 or (result.stdout and "Pyodide error" in result.stdout):
                    error_msg = result.stderr if result.stderr else ""
                    if result.stdout and "Pyodide error" in result.stdout:
                        error_msg = result.stdout.split("Pyodide error:")[-1].strip()
                    return BenchmarkResult(
                        executor_name=self.name,
                        success=False,
                        latency_ms=latency_ms,
                        stdout=result.stdout,
                        stderr=result.stderr,
                        error=error_msg[:200] if error_msg else "Pyodide execution failed"
                    )
                
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
    memory_values = [r.memory_kb / 1024 for r in successful if r.memory_kb > 0]  # Convert KB to MB
    
    if not latencies:
        latencies = [0.0]
    
    avg_memory_mb = statistics.mean(memory_values) if memory_values else 0.0
    peak_memory_mb = max(memory_values) if memory_values else 0.0
    
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
        total_time_sec=total_time,
        avg_memory_mb=avg_memory_mb,
        peak_memory_mb=peak_memory_mb
    )
    
    print(f"\nResults for {executor.name}:")
    print(f"  Success Rate: {len(successful)}/{num_requests} ({len(successful) * 100 // num_requests}%)")
    print(f"  Latency (ms): min={stats.min_latency_ms:.2f}, avg={stats.avg_latency_ms:.2f}, max={stats.max_latency_ms:.2f}")
    print(f"  Percentiles (ms): p50={stats.p50_latency_ms:.2f}, p95={stats.p95_latency_ms:.2f}, p99={stats.p99_latency_ms:.2f}")
    if stats.avg_memory_mb > 0:
        print(f"  Memory (MB): avg={stats.avg_memory_mb:.2f}, peak={stats.peak_memory_mb:.2f}")
    print(f"  Throughput: {stats.throughput_rps:.2f} req/s")
    print(f"  Total Time: {stats.total_time_sec:.2f}s")
    
    if failed:
        error_counts: Dict[str, int] = {}
        for r in failed:
            error = r.error or "Unknown"
            error_counts[error] = error_counts.get(error, 0) + 1
        print(f"  Errors: {error_counts}")
        # Show first few error details for debugging
        if len(failed) > 0:
            first_error = failed[0]
            if first_error.error:
                print(f"  First error detail: {first_error.error[:200]}")
    
    # If all requests failed, show setup error if available
    if len(successful) == 0 and hasattr(executor, 'setup_error') and executor.setup_error:
        print(f"\n  ⚠️  Setup Error: {executor.setup_error}")
        if "Linux" in executor.setup_error:
            print(f"  💡  gVisor only works on Linux. On macOS, use Docker instead.")
        else:
            print(f"  💡  Hint: gVisor requires Docker + runsc installation (Linux only).")
            print(f"      Install: sudo apt-get install runsc && sudo runsc install && sudo systemctl restart docker")
    
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


def print_cold_start_comparison_table(cold_start_results: List[Dict]) -> None:
    """Print cold start benchmark comparison table."""
    if not cold_start_results:
        return
    valid_results = [r for r in cold_start_results if "avg_ms" in r]
    if not valid_results:
        return
    print("\n" + "=" * 90)
    print("COLD START BENCHMARK COMPARISON")
    print("=" * 90)
    headers = ["Executor", "Success%", "Avg(ms)", "Min(ms)", "P50(ms)", "P95(ms)", "Max(ms)"]
    widths = [35, 10, 10, 10, 10, 10, 10]
    header_line = " | ".join(h.ljust(w) for h, w in zip(headers, widths))
    print(header_line)
    print("-" * len(header_line))
    sorted_results = sorted(valid_results, key=lambda r: r["avg_ms"])
    for r in sorted_results:
        success_rate = f"{r['successful'] * 100 // max(1, r.get('iterations', 1))}%"
        row = [
            r["executor"][:35],
            success_rate,
            f"{r['avg_ms']:.1f}",
            f"{r['min_ms']:.1f}",
            f"{r['p50_ms']:.1f}",
            f"{r['p95_ms']:.1f}",
            f"{r['max_ms']:.1f}",
        ]
        print(" | ".join(str(v).ljust(w) for v, w in zip(row, widths)))
    for r in cold_start_results:
        if "error" in r:
            print(f"  {r['executor']}: FAILED - {r['error']}")
    print("=" * 90)
    if len(valid_results) >= 2:
        baseline = sorted_results[0]
        baseline_avg = baseline.get("avg_ms", 0)
        if baseline_avg > 0:
            print(f"\nCold Start Performance (baseline: {baseline['executor']}):")
            for r in sorted_results[1:]:
                ratio = r["avg_ms"] / baseline_avg
                print(f"  {r['executor']}: {ratio:.2f}x slower than baseline")


def generate_test_input() -> str:
    """Generate test input"""
    return json.dumps({
        "operation": "multiply",
        "a": 123,
        "b": 456
    })


def print_comparison_table(all_stats: List[BenchmarkStats]) -> None:
    """Print comparison table"""
    print("\n" + "=" * 120)
    print("BENCHMARK COMPARISON SUMMARY")
    print("=" * 120)
    
    # Check if any stats have memory data
    has_memory = any(s.avg_memory_mb > 0 for s in all_stats)
    
    if has_memory:
        headers = ["Executor", "Success%", "Avg(ms)", "P50(ms)", "P95(ms)", "P99(ms)", "RPS", "Avg(MB)", "Peak(MB)"]
        widths = [35, 10, 10, 10, 10, 10, 10, 10, 10]
    else:
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
        if has_memory:
            if stats.avg_memory_mb > 0:
                row.extend([
                    f"{stats.avg_memory_mb:.1f}",
                    f"{stats.peak_memory_mb:.1f}",
                ])
            else:
                row.extend(["N/A", "N/A"])
        print(" | ".join(str(v).ljust(w) for v, w in zip(row, widths)))
    
    print("=" * 120)
    
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
    parser.add_argument("--include-gvisor", action="store_true", 
                        help="Include gVisor test (NOT RECOMMENDED: runs on Docker, performance will be worse)")
    parser.add_argument("--skip-srt", action="store_true", help="Skip SRT tests")
    parser.add_argument("--skip-pyodide", action="store_true", help="Skip Pyodide tests")
    parser.add_argument("--output", "-o", type=str, help="Output JSON file")
    parser.add_argument("--sandbox-level", "-l", type=int, choices=[1, 2, 3], 
                        help="SkillBox sandbox level (1=no sandbox, 2=sandbox only, 3=sandbox+scan). "
                             "Can also be set via SKILLBOX_SANDBOX_LEVEL env var")
    parser.add_argument("--compare-levels", action="store_true",
                        help="Compare performance across all sandbox levels (1, 2, 3)")
    parser.add_argument("--compare-ipc", action="store_true",
                        help="Include SkillBox IPC (daemon mode) to compare with subprocess. "
                             "Requires SKILLBOX_USE_IPC=1 (set automatically).")
    
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
    
    measure_mem = args.compare_levels
    if args.compare_levels:
        print(f"  Mode: Compare all sandbox levels (1, 2, 3)")
        print(f"  Memory measurement: Enabled")
        # Test all security levels with memory measurement (subprocess)
        executors = [
            SkillBoxExecutor(sandbox_level=1, measure_memory=True),
            SkillBoxExecutor(sandbox_level=2, measure_memory=True),
            SkillBoxExecutor(sandbox_level=3, measure_memory=True),
        ]
        if args.compare_ipc:
            executors.extend([
                SkillBoxIPCExecutor(sandbox_level=1, measure_memory=True),
                SkillBoxIPCExecutor(sandbox_level=2, measure_memory=True),
                SkillBoxIPCExecutor(sandbox_level=3, measure_memory=True),
            ])
            print(f"  IPC comparison: Enabled (SkillBox IPC L1/L2/L3)")
    else:
        print(f"  Sandbox Level: {sandbox_level}")
        executors = [
            SkillBoxExecutor(sandbox_level=sandbox_level, measure_memory=False),
        ]
        if args.compare_ipc:
            executors.append(SkillBoxIPCExecutor(sandbox_level=sandbox_level, measure_memory=measure_mem))
            print(f"  IPC comparison: Enabled (SkillBox IPC)")
    
    if not args.skip_srt:
        executors.append(SRTExecutor(measure_memory=measure_mem))
    
    if not args.skip_pyodide:
        executors.append(PyodideExecutor(measure_memory=measure_mem))
    
    if not args.skip_docker:
        executors.append(DockerExecutor(measure_memory=measure_mem))
    
    # Note: gVisor runs ON TOP OF Docker (--runtime=runsc), so its performance
    # will always be worse than Docker. It's only useful for security isolation
    # comparison, not performance benchmarking. Use security_vs.py for that.
    # Only include if explicitly requested (not recommended for performance tests)
    if args.include_gvisor:
        print("[WARN] gVisor runs ON TOP OF Docker, so its performance will be worse than Docker.")
        print("[WARN] This is mainly for curiosity/completeness, not meaningful performance comparison.")
        executors.append(GvisorExecutor(measure_memory=measure_mem))
    
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
        print_cold_start_comparison_table(cold_start_results)
    
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
