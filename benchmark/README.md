# SkillBox Benchmark Suite

High-concurrency performance comparison test suite for comparing SkillBox with other sandbox solutions.

## Test Targets

| Executor | Description | Isolation Level | Installation |
|----------|-------------|-----------------|--------------|
| **SkillBox (Native Sandbox)** | Native sandbox using Seatbelt/Namespace | System Level | Built-in |
| **SkillBox (No Sandbox)** | SkillBox with sandbox disabled | None | Built-in |
| **Direct Python** | Direct Python script execution | None | Built-in |
| **Subprocess (Resource Limits)** | Subprocess with resource limits | Process Level | Built-in |
| **SRT (Anthropic Sandbox)** | Anthropic open-source sandbox tool | System Level | Requires Installation |
| **Pyodide (WebAssembly)** | Python runtime based on WebAssembly | Browser Level | Requires Installation |
| **Docker Container** | Docker container sandbox | Container Level | Requires Installation |

## Test Metrics

- **Cold Start Latency**
- **Warm Start Latency**
- **Throughput under Concurrency**
- **P50/P95/P99 Latency**
- **Success Rate**
- **Memory Usage**

## Test Scripts

| Script | Description |
|--------|-------------|
| `benchmark_runner.py` | Performance comparison: cold start, high concurrency (SkillBox, Docker, SRT, Pyodide) |
| `security_vs.py` | Security comparison test |
| `security_detailed_vs.py` | Detailed security behavior (blocked vs limited vs allowed) |

## Test Environment

- **Operating System**: macOS
- **SkillBox**: Rust Native Sandbox (Seatbelt)
- **Docker**: python:3.11-slim image (~150MB)
- **Network**: Download 28 Mbps / Upload 28 Mbps

## Installing Dependencies

### Required Dependencies
- Python 3.8+
- SkillBox (Built-in to project, auto-compiled on first run)

### Optional Dependencies (for complete comparison testing)

```bash
# Install psutil for SkillBox IPC memory stats (Avg(MB)/Peak(MB) when using --compare-ipc)
pip install -r requirements.txt
# Or: pip install psutil

# Install SRT (Anthropic Sandbox Runtime)
npm install -g @anthropic-ai/sandbox-runtime

# Install Pyodide (WebAssembly Python)
npm install pyodide

# Install Docker (for container sandbox testing)
# macOS: brew install --cask docker
# Linux: See https://docs.docker.com/engine/install/

```

## Quick Start

**Two ways to run (choose one):**

```bash
# Option 1: From project root
cd /path/to/skillLite
./benchmark/run_benchmark.sh                    # Basic test
python benchmark/benchmark_runner.py -n 100 -c 10  # Or run directly with Python

# Option 2: From benchmark directory
cd /path/to/skillLite/benchmark
./run_benchmark.sh                             # Basic test
python benchmark_runner.py -n 100 -c 10       # No benchmark/ prefix when already in benchmark dir
```

**Note**: When already in the benchmark directory, do not `cd benchmark` again (will error: no such file). Use `python benchmark_runner.py` directly.

```bash
# Full test (IPC comparison, memory stats)
./run_benchmark.sh --compare-levels --compare-ipc
# Or
python benchmark_runner.py --compare-levels --compare-ipc -n 100 -c 10

# Cold start comparison (setup -> execute -> teardown per iteration)
python benchmark_runner.py --cold-start --compare-levels --compare-ipc
python benchmark_runner.py --cold-start --cold-iterations 20 --compare-ipc  # Custom iterations

# Skip Docker / others
./run_benchmark.sh --skip-docker
./run_benchmark.sh -o results.json
```

### Cold Start vs High Concurrency

| Test Type | Description | Use Case |
|----------|------|----------|
| **Cold Start** (`--cold-start`) | Destroy and recreate executor each iteration (IPC daemon is shut down), outputs **cold start comparison table** | Compare first-request latency, IPC vs CMD cold start |
| **High Concurrency** | Keep warm, run multiple requests concurrently, outputs **performance comparison table** | Compare throughput, P50/P95/P99 latency |

Cold start test outputs `COLD START BENCHMARK COMPARISON` table with Avg/Min/P50/P95/Max per Executor and multiplier vs baseline.

### CMD vs IPC Performance

- **High concurrency warm**: CMD (subprocess) and IPC perform similarly; CMD may be slightly faster. For short tasks (~100ms), process creation (~15ms) and IPC JSON/pipe overhead (~10ms) are comparable.
- **Cold start**: IPC has clear advantage—daemon stays resident, avoiding fork/exec per request, lower first-request latency.
- Use `--cold-start --compare-ipc` for direct comparison.
- **IPC Level 3**: Daemon supports concurrency (rayon thread pool); Python client implements batch sending (writer thread + short collection window). First wave of concurrent requests can be batched to the daemon, but later requests arrive one-by-one as jobs finish at different times, so overall throughput remains lower than subprocess. Single batch calls (e.g. `run_skill` loop) benefit from daemon concurrency.

### [INFO] Logging

- **CMD (SkillBox subprocess)**: Uses `capture_output=True`, skillbox stderr is captured so [INFO] does not appear in terminal; benchmark also passes `SKILLBOX_QUIET=1` to reduce logging and syscall overhead.
- **IPC (skillbox serve)**: Daemon sets `SKILLBOX_QUIET=1` at startup; `serve_stdio` enforces it, so no [INFO] during runs to avoid affecting performance tests.

---

## Test Results

### SkillBox vs Docker Warm Start Comparison (Image Cached)

| Test Item | Native Python | SkillBox | Docker | SkillBox Advantage |
|--------|-------------|----------|--------|---------------|
| **startup** | 17.44 ms | 40.14 ms | 194.23 ms | **4.8x faster** |
| simple_print | 17.05 ms | 33.45 ms | 226.56 ms | 6.8x faster |
| loop_1000 | 18.22 ms | 33.54 ms | 228.25 ms | 6.8x faster |
| loop_100000 | 17.84 ms | 34.20 ms | 237.07 ms | 6.9x faster |
| string_ops | 17.35 ms | 33.56 ms | 235.75 ms | 7.0x faster |
| list_comprehension | 17.10 ms | 33.83 ms | 233.85 ms | 6.9x faster |
| fibonacci | 18.01 ms | 34.07 ms | 236.00 ms | 6.9x faster |
| **concurrent_5** | - | 60.63 ms | 417.40 ms | **6.9x faster** |

**Key Conclusions:**
- SkillBox Sandbox Overhead: +22.7 ms (+130%)
- SkillBox vs Docker Startup Speed: **4.8x faster**
- SkillBox vs Docker Concurrent Performance: **6.9x faster**

### Cold Start Comparison (No Cache)

| Environment | Cold Start Time | Description |
|------|-----------|------|
| **SkillBox** | **492 ms** | Local binary loading (~1.6MB) |
| **Docker** | 120,618 ms (2 minutes) | Need to download image (~150MB) |

**🚀 SkillBox cold start is 245x faster than Docker**

## Command Line Arguments

| Argument | Short | Description | Default |
|------|------|------|--------|
| `--requests` | `-n` | Total number of requests | 100 |
| `--concurrency` | `-c` | Concurrency level | 10 |
| `--cold-start` | - | Run cold start test (outputs comparison table) | false |
| `--cold-iterations` | - | Cold start iterations | 10 |
| `--compare-levels` | - | Compare all sandbox levels (1, 2, 3) | false |
| `--compare-ipc` | - | Include SkillBox IPC (daemon mode) vs subprocess | false |
| `--skip-docker` | - | Skip Docker test | false |
| `--output` | `-o` | Output JSON file (includes cold_start_results) | - |

## Test Cases

| Case | Code | Description |
|------|------|------|
| startup | `print("hello")` | Minimum startup time |
| simple_print | `print("Hello, World!")` | Simple output |
| loop_1000 | `sum(range(1000))` | Small loop |
| loop_10000 | `sum(range(10000))` | Medium loop |
| loop_100000 | `sum(range(100000))` | Large loop |
| string_ops | `"hello" * 1000` | String operations |
| list_comprehension | `[x**2 for x in range(1000)]` | List comprehension |
| dict_operations | Dictionary operations | Dictionary CRUD operations |
| fibonacci | Recursive calculation fib(20/25) | CPU intensive |

## Conclusion

| Scenario | SkillBox Advantage | Applicable Situation |
|------|--------------|----------|
| **Cold Start** | 245x faster | First deployment, no cache environment |
| **Warm Start** | 5-7x faster | Daily operation, frequent calls |
| **Concurrent Performance** | 6.9x faster | High-concurrency scenarios |
| **Resource Usage** | Very low | Edge devices, resource-limited environments |
| **Deployment Complexity** | Single binary | No Docker daemon required |

SkillBox's core advantages: **zero dependencies, local execution, millisecond-level startup**.

---

## Pyodide (WebAssembly) Comparison Test

### Test Results

| Test Item | SkillBox (ms) | Pyodide (ms) | SkillBox Advantage |
|--------|---------------|--------------|---------------|
| **startup** | 37.41 | 672.16 | **18x faster** |
| simple_print | 32.60 | 668.08 | 20x faster |
| loop_1000 | 32.62 | 667.52 | 20x faster |
| fibonacci | 32.91 | 673.59 | 20x faster |

**Key Conclusions:**
- SkillBox Startup Time: **37 ms**
- Pyodide Startup Time: **672 ms** (need to load ~50MB WebAssembly)
- **SkillBox is 18-20x faster than Pyodide**

### Running Tests

```bash
python benchmark/benchmark_runner.py --compare-levels --compare-ipc -n 1 -c 1
```

### Pyodide Limitations

Pyodide is a Python sandbox solution used by frameworks like LangChain:

| Dimension | SkillBox | Pyodide |
|------|----------|---------|
| **Runtime** | Native Python | WebAssembly Interpretation |
| **Startup Overhead** | ~40 ms | ~700 ms (loading WASM) |
| **Execution Speed** | Native Speed | 2-5x slower than native |
| **Dependency Size** | 1.6 MB | ~50 MB |
| **Platform Support** | macOS/Linux | Requires Node.js/Browser |

---

## SRT (Anthropic Sandbox Runtime) Comparison Test

SRT is Anthropic's open-source sandbox runtime that uses the same underlying technology (Seatbelt) but implemented in Rust.

### Test Results

| Test Item | SkillBox (ms) | SRT (ms) | SkillBox Advantage |
|--------|---------------|----------|---------------|
| **startup** | 119.91 | 596.00 | **5.0x faster** |
| simple_print | 121.50 | 717.36 | 5.9x faster |
| loop_10000 | 119.98 | 713.05 | 5.9x faster |
| fibonacci_25 | 120.78 | 720.48 | 6.0x faster |
| list_comprehension | 119.99 | 718.69 | 6.0x faster |
| dict_operations | 120.63 | 720.52 | 6.0x faster |

**Key Conclusions:**
- SkillBox Startup Time: **120 ms**
- SRT Startup Time: **596 ms**
- **SkillBox is approximately 5-6x faster than SRT**

### Memory Usage Comparison

| Test Item | SkillBox (KB) | SRT (KB) | SkillBox Advantage |
|--------|---------------|----------|---------------|
| startup | 12,208 | 84,416 | **6.9x lower** |
| simple_print | 12,192 | 84,304 | 6.9x lower |
| loop_10000 | 12,208 | 83,552 | 6.8x lower |
| fibonacci_25 | 12,272 | 82,848 | 6.8x lower |

### Security Comparison

| Security Test Item | SkillBox | SRT |
|-----------|----------|-----|
| Read /etc/passwd | ✅ Blocked | ❌ Allowed |
| Network Access | ✅ Blocked | ✅ Blocked |
| Process Creation | ✅ Blocked | ❌ Allowed |
| Write to /tmp | ✅ Blocked | ✅ Blocked |

### Running Tests

```bash
python benchmark/benchmark_runner.py --compare-levels --compare-ipc -n 1 -c 1
```

> Reference: [Anthropic Sandbox Runtime](https://github.com/anthropics/anthropic-quickstarts)

---

## Security Comparison Test

In addition to performance tests, we provide security comparison tests to evaluate the protection capabilities of sandbox solutions against malicious behavior.

### Test Dimensions

| Category | Test Item | Description |
|------|--------|------|
| **File System** | Read sensitive files | `/etc/passwd`, `~/.ssh/id_rsa` |
| | Write files | Try to create files outside sandbox |
| | Directory traversal | `../../../` path traversal attacks |
| **Network** | HTTP requests | External network access capability |
| | DNS queries | Domain name resolution capability |
| | Port listening | Open socket services |
| **Process** | System commands | `os.system()`, `subprocess` |
| | Process enumeration | View other process information |
| | Signal sending | Try to kill other processes |
| **Resource Limits** | Memory bomb | Infinite memory allocation |
| | Fork bomb | Infinite process creation |
| | CPU bomb | Infinite loop calculation |
| **Code Injection** | Dynamic import | `__import__`, `importlib` |
| | eval/exec | Dynamic code execution |

### Security Comparison Results

| Test Item               |    SkillBox    |     Docker     |    Pyodide     |   Claude SRT   |
|----------------------|----------------|----------------|----------------|----------------|
| **File System** | | | | |
| Read /etc/passwd       |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| Read SSH private key    |      ✅ Blocked      |      ✅ Blocked      |      ✅ Blocked      |      ❌ Allowed      |
| Write to /tmp dir       |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Directory traversal     |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| List root directory     |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| **Network** | | | | |
| Send HTTP request       |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ✅ Blocked      |
| DNS query              |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Listen port             |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| **Process** | | | | |
| Execute os.system()    |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Execute subprocess     |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| Enumerate processes    |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Send process signal    |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |    ⚠️ Partially Blocked     |
| **Resource Limits** | | | | |
| Memory bomb             |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Fork bomb              |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| CPU intensive compute  |      ✅ Blocked      |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |
| **Code Injection** | | | | |
| Dynamic import os      |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Use eval/exec          |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Modify built-in funcs  |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| **Information Leakage** | | | | |
| Read environment vars  |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Get system info        |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |

#### Security Scores

| Platform | Blocked | Partially Blocked | Allowed | Security Score |
|------|------|----------|------|----------|
| SkillBox | 18 | 0 | 2 | 90.0% |
| Docker | 2 | 0 | 18 | 10.0% |
| Pyodide | 7 | 0 | 13 | 35.0% |
| Claude SRT | 6 | 1 | 13 | 32.5% |

### Running Security Tests

```bash
# Complete test (SkillBox + Docker + Pyodide)
python3 benchmark/security_vs.py

# Test SkillBox only
python3 benchmark/security_vs.py --skip-docker --skip-pyodide

# Output JSON results
python3 benchmark/security_vs.py --output security_results.json
```

### Parameter Description

| Argument | Description | Default |
|------|------|--------|
| `--skillbox` | SkillBox executable path | Auto-detect |
| `--docker-image` | Docker image name | python:3.11-slim |
| `--skip-docker` | Skip Docker test | false |
| `--skip-pyodide` | Skip Pyodide test | false |
| `--output` | Output JSON result file path | - |

### Result Description

| Symbol | Meaning |
|------|------|
| ✅ Blocked | Attack completely blocked |
| ⚠️ Partially Blocked | Attack partially blocked or limited |
| ❌ Allowed | Attack executed successfully |
| ⏭️ Skipped | Test skipped |

---

## Comprehensive Comparison Summary

| Dimension | SkillBox | Docker | Pyodide | SRT |
|------|----------|--------|---------|-----|
| **Warm Start Latency** | 40 ms | 194 ms | 672 ms | 596 ms |
| **Cold Start Latency** | 492 ms | 120s | ~5s | ~1s |
| **Memory Usage** | 10 MB | ~100 MB | ~50 MB | 84 MB |
| **Security** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **Deployment Complexity** | Single binary | Requires daemon | Requires Node.js | Requires installation |
| **Platform Support** | macOS/Linux | All platforms | All platforms | macOS/Linux |

**Note**: gVisor runs ON TOP OF Docker (using `--runtime=runsc`), so its performance will always be worse than Docker. It's only useful for security isolation comparison, not performance benchmarking. See `security_vs.py` for security comparison tests.

---

## Extended Testing

To add new executors for comparison, you can inherit the `BaseExecutor` class:

```python
class MyCustomExecutor(BaseExecutor):
    name = "My Custom Executor"
    
    def setup(self) -> None:
        # Initialize
        pass
    
    def execute(self, input_json: str) -> BenchmarkResult:
        # Execution logic
        pass
    
    def teardown(self) -> None:
        # Cleanup
        pass
```

## Notes

1. **Docker Test**: Requires Docker installation and user permission to run Docker commands
2. **gVisor**: gVisor runs ON TOP OF Docker (using `--runtime=runsc`), so its performance will always be worse than Docker. It's only useful for security isolation comparison, not performance benchmarking. Use `security_vs.py` for security comparison tests.
3. **SkillBox Compilation**: Auto-compiled on first run (requires Rust environment)
4. **Resource Limits**: `Subprocess (Resource Limits)` uses `resource` module, only available on Unix systems
5. **Result Fluctuation**: Recommended to run multiple times and take average to avoid system load impact

Security score formula: `(Blocked Count + Partially Blocked Count × 0.5) / Total Test Count × 100%`

Higher scores indicate better sandbox security. Native Python has no sandbox protection, score close to 0%, as benchmark comparison.

### Current Status Description

**macOS Platform Limitations**:

Due to macOS System Integrity Protection (SIP) limitations, `sandbox-exec` may not work properly on modern macOS versions. SkillBox uses the following strategy:

1. **Try sandbox-exec first**: Use Seatbelt profile for sandbox isolation
2. **Fall back to restricted execution**: If sandbox-exec fails, use environment isolation:
   - Clear sensitive environment variables
   - Set isolated HOME and TMPDIR
   - Disable Python user site packages

**Linux Platform**:

Stronger isolation mechanisms are used on Linux:
- **bubblewrap (bwrap)**: Recommended, provides complete namespace isolation
- **firejail**: Alternative, provides seccomp and file system isolation
- **Namespace isolation**: Requires root privilege

### Install Recommended Sandbox Tools

**Linux**:
```bash
# Ubuntu/Debian
sudo apt install bubblewrap

# Or
sudo apt install firejail
```

**macOS**:
macOS uses built-in sandbox-exec; no additional installation required.

### Disable Sandbox

If you need to disable sandbox (not recommended), set the environment variable:
```bash
export SKILLBOX_NO_SANDBOX=1
```
