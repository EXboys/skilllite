# 02. Skill Management

## Core Examples

### list_skills.py
List all available skills and their information

```bash
python list_skills.py
```

Displays:
- Skill name and description
- Programming language
- Entry point file

### execute_skill.py
Execute a skill directly (without LLM)

```bash
python execute_skill.py
```

## Basic Code Template

```python
from skilllite import run_skill

# Execute a skill directly (CLI: skilllite list, skilllite run)
result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

> 如需 SkillManager（列出、执行等），请使用 `pip install langchain-skilllite`：
> ```python
> from langchain_skilllite import SkillManager
> manager = SkillManager(skills_dir="./skills")
> for skill in manager.list_skills(): ...
> result = manager.execute("calculator", {...})
> ```

## Next Steps

- [03. Agentic Loop](../03_agentic_loop/README.md)
- [04. LangChain Integration](../04_langchain_integration/README.md)
