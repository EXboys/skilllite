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
from skilllite import SkillManager

manager = SkillManager(skills_dir="./skills")

# List all skills
skills = manager.list_skills()
for skill in skills:
    print(f"{skill.name}: {skill.description}")

# Execute a skill
result = manager.execute("skill_name", {"param": "value"})
if result.success:
    print(result.output)
else:
    print(f"Error: {result.error}")
```

## Next Steps

- [03. Agentic Loop](../03_agentic_loop/README.md)
- [04. LangChain Integration](../04_langchain_integration/README.md)
