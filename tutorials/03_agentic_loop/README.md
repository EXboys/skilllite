# 03. Agentic Loop

## Core Examples

### basic_loop.py
Complete agent loop example

Includes two implementations:
1. **Manual Agent Loop** - No framework dependency, uses OpenAI API directly
2. **SkillLite Agent** - Uses SkillLite built-in Agent (recommended)

```bash
python basic_loop.py
```

## Basic Code Template

### Simplest Approach (Recommended)
```python
from skilllite import chat

result = chat("Your request", skills_dir=".skills")
print(result)
```

### LangChain Agent
如需更灵活的 Agent 控制，请使用 [langchain-skilllite](../04_langchain_integration/README.md)。

## Next Steps

- [04. LangChain Integration](../04_langchain_integration/README.md)
- [05. LlamaIndex Integration](../05_llamaindex_integration/README.md)
