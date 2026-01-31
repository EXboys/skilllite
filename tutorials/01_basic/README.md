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

### SkillRunner
```python
from skilllite import SkillRunner

runner = SkillRunner()
result = runner.run("Your request")
```

### SkillManager
```python
from skilllite import SkillManager

manager = SkillManager(skills_dir="./skills")
result = manager.execute("skill_name", {"param": "value"})
```

## Next Steps

- [02. Skill Management](../02_skill_management/README.md)
- [03. Agentic Loop](../03_agentic_loop/README.md)
- [04. LangChain Integration](../04_langchain_integration/README.md)

