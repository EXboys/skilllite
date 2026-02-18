# 快速入门

## 安装

### 1. 安装 Python SDK

```bash
pip install skilllite
```

### 2. 初始化项目

```bash
# 安装沙箱二进制并创建 .skills/ 目录
skilllite init

# 验证安装
skilllite status
```

或手动安装：
```bash
curl -fsSL https://raw.githubusercontent.com/EXboys/skilllite/main/install.sh | bash
```

**支持的平台：**
- macOS (Intel 和 Apple Silicon)
- Linux (x86_64 和 ARM64)

### 3. 验证安装

```bash
skilllite status
```

## 快速使用

### 基础示例

```python
from skilllite import chat

# 单次 Agent 对话（使用 .env 中的 API 配置）
result = chat("计算 15 乘以 23", skills_dir=".skills")
print(result)
```

LangChain/LlamaIndex 集成请使用 `langchain-skilllite`：
```bash
pip install langchain-skilllite
```

### 支持的 LLM 提供商

| 提供商 | base_url |
|--------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| 通义千问 | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| 月之暗面 | `https://api.moonshot.cn/v1` |
| Ollama | `http://localhost:11434/v1` |

## CLI 命令

```bash
skilllite init             # 初始化项目（沙箱 + .skills/）
skilllite init --skip-deps # 跳过依赖安装
skilllite status           # 检查安装状态
skilllite add owner/repo   # 从 GitHub 添加 skills
skilllite list             # 列出已安装的 skills
skilllite chat             # 交互式 Agent 对话
skilllite mcp              # 启动 MCP 服务器 (需要 pip install skilllite[mcp])
```

## 创建 Skill

```
my-skill/
├── SKILL.md           # 必需：元数据和文档
├── scripts/
│   └── main.py        # 入口脚本
├── references/        # 可选：参考文档
└── assets/            # 可选：资源文件
```

### SKILL.md 示例

```markdown
---
name: my-skill
description: 我的自定义 Skill
compatibility: Requires Python 3.x with requests library, network access
license: MIT
---

# My Skill

这个 Skill 可以做一些有用的事情。
```

## 故障排除

### 找不到二进制文件

```bash
echo 'export PATH="$HOME/.skilllite/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### 手动下载

如果自动安装失败，请从以下地址下载：https://github.com/EXboys/skilllite/releases

### 从源码构建

```bash
git clone https://github.com/EXboys/skilllite.git
cd skilllite/skilllite
cargo build --release
cargo install --path .
```

## 下一步

- 阅读 [架构指南](./ARCHITECTURE.md) 了解详细设计
- 查看 [贡献指南](./CONTRIBUTING.md) 了解如何贡献
- 探索 [benchmark/](../../benchmark/) 了解性能测试

