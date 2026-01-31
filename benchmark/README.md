# SkillBox Benchmark Suite

高并发性能对比测试套件，用于对比 SkillBox 与其他沙箱方案的性能表现。

## 测试对象

| 执行器 | 描述 | 隔离级别 | 安装要求 |
|--------|------|----------|----------|
| **SkillBox (Native Sandbox)** | 使用 Seatbelt/Namespace 的原生沙箱 | 系统级 | 内置 |
| **SkillBox (No Sandbox)** | 禁用沙箱的 SkillBox | 无 | 内置 |
| **Direct Python** | 直接执行 Python 脚本 | 无 | 内置 |
| **Subprocess (Resource Limits)** | 带资源限制的子进程 | 进程级 | 内置 |
| **SRT (Anthropic Sandbox)** | Anthropic 开源的沙箱工具 | 系统级 | 需安装 |
| **Pyodide (WebAssembly)** | 基于 WebAssembly 的 Python 运行时 | 浏览器级 | 需安装 |
| **Docker Container** | Docker 容器沙箱 | 容器级 | 需安装 |

## 测试指标

- **冷启动时间** (Cold Start Latency)
- **热启动时间** (Warm Start Latency)
- **并发吞吐量** (Throughput under Concurrency)
- **P50/P95/P99 延迟**
- **成功率**
- **内存占用**

## 测试脚本

| 脚本 | 对比对象 | 说明 |
|------|----------|------|
| `benchmark_runner.py` | 全部执行器 | 高并发性能对比 |
| `docker_vs.py` | Docker | 容器 vs 原生沙箱 |
| `pyodide_vs.py` | Pyodide (WebAssembly) | WASM vs 原生沙箱 |
| `srt_vs_skillbox_benchmark.py` | SRT (Anthropic) | Anthropic 沙箱对比 |
| `security_vs.py` | 全部 | 安全性对比测试 |

## 测试环境

- **操作系统**: macOS
- **SkillBox**: Rust 原生沙箱 (Seatbelt)
- **Docker**: python:3.11-slim 镜像 (~150MB)
- **网络**: 下载 28 Mbps / 上传 28 Mbps

## 安装依赖

### 必需依赖
- Python 3.8+
- SkillBox（项目内置，首次运行会自动编译）

### 可选依赖（用于完整对比测试）

```bash
# 安装 SRT (Anthropic Sandbox Runtime)
npm install -g @anthropic-ai/sandbox-runtime

# 安装 Pyodide (WebAssembly Python)
npm install pyodide

# 安装 Docker（用于容器沙箱测试）
# macOS: brew install --cask docker
# Linux: 参考 https://docs.docker.com/engine/install/
```

## 快速开始

```bash
# 基础测试 (100 请求, 10 并发)
./run_benchmark.sh

# 高并发测试 (500 请求, 50 并发)
./run_benchmark.sh -n 500 -c 50

# 包含冷启动测试
./run_benchmark.sh --cold-start

# 跳过 Docker 测试
./run_benchmark.sh --skip-docker

# 保存结果到文件
./run_benchmark.sh -o results.json
```

## 直接使用 Python

```bash
# 基础测试
python3 benchmark_runner.py -n 100 -c 10

# 完整测试
python3 benchmark_runner.py -n 500 -c 50 --cold-start -o results.json
```

---

## 测试结果

### SkillBox vs Docker 热启动对比 (镜像已缓存)

| 测试项 | 原生 Python | SkillBox | Docker | SkillBox 优势 |
|--------|-------------|----------|--------|---------------|
| **startup** | 17.44 ms | 40.14 ms | 194.23 ms | **4.8x 更快** |
| simple_print | 17.05 ms | 33.45 ms | 226.56 ms | 6.8x 更快 |
| loop_1000 | 18.22 ms | 33.54 ms | 228.25 ms | 6.8x 更快 |
| loop_100000 | 17.84 ms | 34.20 ms | 237.07 ms | 6.9x 更快 |
| string_ops | 17.35 ms | 33.56 ms | 235.75 ms | 7.0x 更快 |
| list_comprehension | 17.10 ms | 33.83 ms | 233.85 ms | 6.9x 更快 |
| fibonacci | 18.01 ms | 34.07 ms | 236.00 ms | 6.9x 更快 |
| **concurrent_5** | - | 60.63 ms | 417.40 ms | **6.9x 更快** |

**关键结论:**
- SkillBox 沙箱开销: +22.7 ms (+130%)
- SkillBox vs Docker 启动速度: **4.8x 更快**
- SkillBox vs Docker 并发性能: **6.9x 更快**

### 冷启动对比 (无缓存)

| 环境 | 冷启动时间 | 说明 |
|------|-----------|------|
| **SkillBox** | **492 ms** | 本地二进制加载 (~1.6MB) |
| **Docker** | 120,618 ms (2分钟) | 需下载镜像 (~150MB) |

**🚀 SkillBox 比 Docker 冷启动快 245x**

## 命令行参数

| 参数 | 简写 | 描述 | 默认值 |
|------|------|------|--------|
| `--requests` | `-n` | 请求总数 | 100 |
| `--concurrency` | `-c` | 并发数 | 10 |
| `--cold-start` | - | 运行冷启动测试 | false |
| `--cold-iterations` | - | 冷启动迭代次数 | 10 |
| `--skip-docker` | - | 跳过 Docker 测试 | false |
| `--output` | `-o` | 输出 JSON 文件 | - |

## 测试用例

| 用例 | 代码 | 说明 |
|------|------|------|
| startup | `print("hello")` | 最小启动时间 |
| simple_print | `print("Hello, World!")` | 简单输出 |
| loop_1000 | `sum(range(1000))` | 小循环 |
| loop_10000 | `sum(range(10000))` | 中循环 |
| loop_100000 | `sum(range(100000))` | 大循环 |
| string_ops | `"hello" * 1000` | 字符串操作 |
| list_comprehension | `[x**2 for x in range(1000)]` | 列表推导 |
| dict_operations | 字典操作 | 字典增删改查 |
| fibonacci | 递归计算 fib(20/25) | CPU 密集型 |

## 结论

| 场景 | SkillBox 优势 | 适用情况 |
|------|--------------|----------|
| **冷启动** | 245x 更快 | 首次部署、无缓存环境 |
| **热启动** | 5-7x 更快 | 日常运行、高频调用 |
| **并发性能** | 6.9x 更快 | 高并发场景 |
| **资源占用** | 极低 | 边缘设备、资源受限环境 |
| **部署复杂度** | 单二进制 | 无需 Docker 守护进程 |

SkillBox 的核心优势在于：**零依赖、本地执行、毫秒级启动**。

---

## Pyodide (WebAssembly) 对比测试

### 测试结果

| 测试项 | SkillBox (ms) | Pyodide (ms) | SkillBox 优势 |
|--------|---------------|--------------|---------------|
| **startup** | 37.41 | 672.16 | **18x 更快** |
| simple_print | 32.60 | 668.08 | 20x 更快 |
| loop_1000 | 32.62 | 667.52 | 20x 更快 |
| fibonacci | 32.91 | 673.59 | 20x 更快 |

**关键结论:**
- SkillBox 启动时间: **37 ms**
- Pyodide 启动时间: **672 ms** (需加载 ~50MB WebAssembly)
- **SkillBox 比 Pyodide 快 18-20x**

### 运行测试

```bash
python3 benchmark/pyodide_vs.py
```

### Pyodide 的局限性

Pyodide 是 LangChain 等框架使用的 Python 沙箱方案：

| 维度 | SkillBox | Pyodide |
|------|----------|---------|
| **运行时** | 原生 Python | WebAssembly 解释执行 |
| **启动开销** | ~40 ms | ~700 ms (加载 WASM) |
| **执行速度** | 原生速度 | 比原生慢 2-5x |
| **依赖大小** | 1.6 MB | ~50 MB |
| **平台支持** | macOS/Linux | 需要 Node.js/浏览器 |

---

## SRT (Anthropic Sandbox Runtime) 对比测试

SRT 是 Anthropic 开源的沙箱运行时，使用相同的底层技术 (Seatbelt)，但用 Rust 实现。

### 测试结果

| 测试项 | SkillBox (ms) | SRT (ms) | SkillBox 优势 |
|--------|---------------|----------|---------------|
| **startup** | 119.91 | 596.00 | **5.0x 更快** |
| simple_print | 121.50 | 717.36 | 5.9x 更快 |
| loop_10000 | 119.98 | 713.05 | 5.9x 更快 |
| fibonacci_25 | 120.78 | 720.48 | 6.0x 更快 |
| list_comprehension | 119.99 | 718.69 | 6.0x 更快 |
| dict_operations | 120.63 | 720.52 | 6.0x 更快 |

**关键结论:**
- SkillBox 启动时间: **120 ms**
- SRT 启动时间: **596 ms**
- **SkillBox 比 SRT 快约 5-6x**

### 内存占用对比

| 测试项 | SkillBox (KB) | SRT (KB) | SkillBox 优势 |
|--------|---------------|----------|---------------|
| startup | 12,208 | 84,416 | **6.9x 更低** |
| simple_print | 12,192 | 84,304 | 6.9x 更低 |
| loop_10000 | 12,208 | 83,552 | 6.8x 更低 |
| fibonacci_25 | 12,272 | 82,848 | 6.8x 更低 |

### 安全性对比

| 安全测试项 | SkillBox | SRT |
|-----------|----------|-----|
| 读取 /etc/passwd | ✅ 阻止 | ❌ 允许 |
| 网络访问 | ✅ 阻止 | ✅ 阻止 |
| 进程创建 | ✅ 阻止 | ❌ 允许 |
| 写入 /tmp | ✅ 阻止 | ✅ 阻止 |

### 运行测试

```bash
python3 benchmark/srt_vs_skillbox_benchmark.py
```

> 参考: [Anthropic Sandbox Runtime](https://github.com/anthropics/anthropic-quickstarts)

---

## 安全性对比测试

除了性能测试，我们还提供了安全性对比测试，用于评估各沙箱方案对恶意行为的防护能力。

### 测试维度

| 类别 | 测试项 | 说明 |
|------|--------|------|
| **文件系统** | 读取敏感文件 | `/etc/passwd`、`~/.ssh/id_rsa` |
| | 写入文件 | 尝试在沙箱外创建文件 |
| | 目录遍历 | `../../../` 路径穿越攻击 |
| **网络** | HTTP 请求 | 外网访问能力 |
| | DNS 查询 | 域名解析能力 |
| | 端口监听 | 开启 socket 服务 |
| **进程** | 系统命令 | `os.system()`、`subprocess` |
| | 进程枚举 | 查看其他进程信息 |
| | 信号发送 | 尝试 kill 其他进程 |
| **资源限制** | 内存炸弹 | 无限分配内存 |
| | Fork 炸弹 | 无限创建进程 |
| | CPU 炸弹 | 无限循环计算 |
| **代码注入** | 动态导入 | `__import__`、`importlib` |
| | eval/exec | 动态代码执行 |

### 安全性对比结果

| 测试项                  |    Skillbox    |     Docker     |    Pyodide     |   Claude SRT   |
|----------------------|----------------|----------------|----------------|----------------|
| **文件系统** | | | | |
| 读取 /etc/passwd       |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |      ❌ 允许      |
| 读取 SSH 私钥            |      ✅ 阻止      |      ✅ 阻止      |      ✅ 阻止      |      ❌ 允许      |
| 写入 /tmp 目录           |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ✅ 阻止      |
| 目录遍历攻击 (../../../)   |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |      ❌ 允许      |
| 列出根目录内容              |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| **网络** | | | | |
| 发起 HTTP 请求           |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |      ✅ 阻止      |
| DNS 查询               |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ✅ 阻止      |
| 监听端口                 |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ✅ 阻止      |
| **进程** | | | | |
| 执行 os.system()       |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| 执行 subprocess        |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |      ❌ 允许      |
| 枚举系统进程               |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ✅ 阻止      |
| 发送进程信号               |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |    ⚠️ 部分阻止     |
| **资源限制** | | | | |
| 内存炸弹 (分配大量内存)        |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| Fork 炸弹              |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |      ❌ 允许      |
| CPU 密集计算 (是否有时间限制)   |      ✅ 阻止      |      ✅ 阻止      |      ❌ 允许      |      ✅ 阻止      |
| **代码注入** | | | | |
| 动态导入 os 模块           |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| 使用 eval/exec 执行代码    |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| 修改内置函数               |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| **信息泄露** | | | | |
| 读取环境变量               |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |
| 获取系统信息               |      ✅ 阻止      |      ❌ 允许      |      ❌ 允许      |      ❌ 允许      |

#### 安全评分

| 平台 | 阻止 | 部分阻止 | 允许 | 安全评分 |
|------|------|----------|------|----------|
| Skillbox | 18 | 0 | 2 | 90.0% |
| Docker | 2 | 0 | 18 | 10.0% |
| Pyodide | 7 | 0 | 13 | 35.0% |
| Claude SRT | 6 | 1 | 13 | 32.5% |

### 运行安全性测试

```bash
# 完整测试 (SkillBox + Docker + Pyodide)
python3 benchmark/security_vs.py

# 仅测试 SkillBox
python3 benchmark/security_vs.py --skip-docker --skip-pyodide

# 输出 JSON 结果
python3 benchmark/security_vs.py --output security_results.json
```

### 参数说明

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `--skillbox` | SkillBox 可执行文件路径 | 自动检测 |
| `--docker-image` | Docker 镜像名称 | python:3.11-slim |
| `--skip-docker` | 跳过 Docker 测试 | false |
| `--skip-pyodide` | 跳过 Pyodide 测试 | false |
| `--output` | 输出 JSON 结果文件路径 | - |

### 结果说明

| 符号 | 含义 |
|------|------|
| ✅ 阻止 | 攻击被完全阻止 |
| ⚠️ 部分阻止 | 攻击被部分阻止或有限制 |
| ❌ 允许 | 攻击成功执行 |
| ⏭️ 跳过 | 测试被跳过 |

---

## 综合对比总结

| 维度 | SkillBox | Docker | Pyodide | SRT |
|------|----------|--------|---------|-----|
| **热启动延迟** | 40 ms | 194 ms | 672 ms | 596 ms |
| **冷启动延迟** | 492 ms | 120s | ~5s | ~1s |
| **内存占用** | 12 MB | ~100 MB | ~50 MB | 84 MB |
| **安全性** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **部署复杂度** | 单二进制 | 需守护进程 | 需 Node.js | 需安装 |
| **平台支持** | macOS/Linux | 全平台 | 全平台 | macOS/Linux |

---

## 扩展测试

如需添加新的执行器进行对比，可以继承 `BaseExecutor` 类：

```python
class MyCustomExecutor(BaseExecutor):
    name = "My Custom Executor"
    
    def setup(self) -> None:
        # 初始化
        pass
    
    def execute(self, input_json: str) -> BenchmarkResult:
        # 执行逻辑
        pass
    
    def teardown(self) -> None:
        # 清理
        pass
```

## 注意事项

1. **Docker 测试**：需要安装 Docker 并确保当前用户有权限运行 Docker 命令
2. **SkillBox 编译**：首次运行会自动编译 SkillBox（需要 Rust 环境）
3. **资源限制**：`Subprocess (Resource Limits)` 使用 `resource` 模块，仅在 Unix 系统可用
4. **结果波动**：建议多次运行取平均值，避免系统负载影响结果

安全评分计算公式：`(阻止数 + 部分阻止数 × 0.5) / 总测试数 × 100%`

评分越高表示沙箱安全性越好。原生 Python 无沙箱保护，评分接近 0%，作为基准对比。

### 当前状态说明

**macOS 平台限制**：

由于 macOS 的系统完整性保护 (SIP) 限制，`sandbox-exec` 在现代 macOS 版本上可能无法正常工作。Skillbox 采用以下策略：

1. **优先尝试 sandbox-exec**：使用 Seatbelt profile 进行沙箱隔离
2. **回退到受限执行**：如果 sandbox-exec 失败，使用环境隔离：
   - 清除敏感环境变量
   - 设置隔离的 HOME 和 TMPDIR
   - 禁用 Python 用户站点包

**Linux 平台**：

Linux 上使用更强的隔离机制：
- **bubblewrap (bwrap)**：推荐，提供完整的命名空间隔离
- **firejail**：备选，提供 seccomp 和文件系统隔离
- **命名空间隔离**：需要 root 权限

### 安装推荐的沙箱工具

**Linux**:
```bash
# Ubuntu/Debian
sudo apt install bubblewrap

# 或者
sudo apt install firejail
```

**macOS**:
macOS 使用内置的 sandbox-exec，无需额外安装。

### 禁用沙箱

如果需要禁用沙箱（不推荐），设置环境变量：
```bash
export SKILLBOX_NO_SANDBOX=1
```
