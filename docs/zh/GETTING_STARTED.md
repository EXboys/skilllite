# 快速入门

## 安装

### 1. 安装 Python SDK

```bash
pip install skilllite
```

### 2. 安装沙箱二进制文件

SkillLite 使用基于 Rust 的沙箱来安全执行代码。

```bash
# 通过 CLI 自动安装
skilllite install

# 或手动安装
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
from skilllite import SkillManager
from openai import OpenAI

# 初始化
client = OpenAI(base_url="https://api.deepseek.com/v1", api_key="your_key")
manager = SkillManager(skills_dir="./.skills")

# 获取工具定义
tools = manager.get_tools()

# 调用 LLM
response = client.chat.completions.create(
    model="deepseek-chat",
    tools=tools,
    messages=[{"role": "user", "content": "计算 15 乘以 23"}]
)

# 处理工具调用
if response.choices[0].message.tool_calls:
    results = manager.handle_tool_calls(response)
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
skilllite install          # 安装沙箱二进制文件
skilllite install --force  # 强制重新安装
skilllite status           # 检查安装状态
skilllite version          # 显示版本信息
skilllite uninstall        # 卸载二进制文件
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
echo 'export PATH="$HOME/.skillbox/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### 手动下载

如果自动安装失败，请从以下地址下载：https://github.com/EXboys/skilllite/releases

### 从源码构建

```bash
git clone https://github.com/EXboys/skilllite.git
cd skilllite/skillbox
cargo build --release
cargo install --path .
```

## 下一步

- 阅读 [架构指南](./ARCHITECTURE.md) 了解详细设计
- 查看 [贡献指南](./CONTRIBUTING.md) 了解如何贡献
- 探索 [benchmark/](../../benchmark/) 了解性能测试

