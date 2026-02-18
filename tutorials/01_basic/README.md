# 01. Basic Usage

## Quick Start (5 minutes)

### hello_world.py
Simplest example, one-line execution
```bash
python hello_world.py
```

### direct_execution.py
Execute skills directly without LLM
```bash
python direct_execution.py
```

### tool_definitions.py
View tool definitions in different formats (Claude vs OpenAI)
```bash
python tool_definitions.py
```

## Core Concepts

### chat API (Agent 对话)
```python
from skilllite import chat

result = chat("Your request", skills_dir=".skills")
print(result)
```

### 直接执行 Skill (无 LLM)
```python
from skilllite import run_skill

result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

> 如需 LangChain 集成，请使用 `pip install langchain-skilllite`，参见 [04. LangChain Integration](../04_langchain_integration/README.md)。

## Next Steps

- [02. Skill Management](../02_skill_management/README.md)
- [03. Agentic Loop](../03_agentic_loop/README.md)
- [04. LangChain Integration](../04_langchain_integration/README.md)

