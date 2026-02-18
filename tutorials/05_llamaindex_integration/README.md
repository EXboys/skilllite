# 05. LlamaIndex Integration

> **Note**: LlamaIndex 适配器已从主仓库移除。当前推荐使用 [langchain-skilllite](https://pypi.org/project/langchain-skilllite/) 进行 Agent 集成，或通过 SkillLite MCP Server（参见 [06. MCP Server](../06_mcp_server/README.md)）与支持 MCP 的 IDE 配合使用。

## 替代方案

### 方案 1：使用 LangChain 集成

```bash
pip install langchain-skilllite langchain-openai
```

参见 [04. LangChain Integration](../04_langchain_integration/README.md)。

### 方案 2：使用 MCP Server

SkillLite 提供 MCP 协议支持，可与 Cursor、OpenCode 等 IDE 集成。参见 [06. MCP Server](../06_mcp_server/README.md)。

### 方案 3：直接调用 skilllite CLI

```python
import subprocess
from skilllite import get_binary

binary = get_binary()
result = subprocess.run(
    [binary, "run", "./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}'],
    capture_output=True, text=True,
)
```

## Next Steps

- [04. LangChain Integration](../04_langchain_integration/README.md) (for comparison)
- [06. MCP Server](../06_mcp_server/README.md)
