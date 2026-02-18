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

## CLI 命令参考（skilllite）

Python SDK 和 tutorials 底层均通过调用 skilllite CLI（Rust 二进制）完成操作。**新增 skill 或集成时请参考以下命令**。

### 技能管理

| 命令 | 说明 | 示例 |
|------|------|------|
| `skilllite list` | 列出已安装的 skills | `skilllite list -s .skills --json` |
| `skilllite add` | 从 GitHub/本地添加 skill | `skilllite add owner/repo` |
| `skilllite remove` | 移除已安装的 skill | `skilllite remove my-skill` |
| `skilllite show` | 查看 skill 详情 | `skilllite show my-skill -s .skills` |

### 技能执行

| 命令 | 说明 | 示例 |
|------|------|------|
| `skilllite run` | 执行 skill（需 SKILL.md 中 entry_point） | `skilllite run ./.skills/calculator '{"operation": "add", "a": 1, "b": 2}'` |
| `skilllite exec` | 直接执行指定脚本（无需 entry_point） | `skilllite exec ./.skills/my-skill scripts/main.py '{}'` |
| `skilllite bash` | 执行 bash-tool 类 skill 的命令 | `skilllite bash ./.skills/browser-tool 'agent-browser --help'` |

### 工具与适配器

| 命令 | 说明 | 示例 |
|------|------|------|
| `skilllite list-tools` | 输出 OpenAI/Claude 格式的 tool 定义（供 LLM 使用） | `skilllite list-tools -s .skills --format openai` |

### 常用选项

```bash
# list：指定 skills 目录，JSON 输出（tutorials 中 list_skills.py / direct_execution.py 使用）
skilllite list -s .skills --json

# run：指定 skill 目录和输入 JSON
skilllite run ./.skills/calculator '{"operation": "add", "a": 15, "b": 27}'

# run：可选参数
skilllite run <SKILL_DIR> '<INPUT_JSON>' [--allow-network] [--timeout 60] [--sandbox-level 3]

# list-tools：tool_definitions.py 使用
skilllite list-tools -s .skills --format openai   # 或 --format claude
```

### 其他常用命令

```bash
skilllite init              # 初始化项目（沙箱 + .skills/）
skilllite init --skip-deps  # 跳过依赖安装
skilllite validate <SKILL_DIR>   # 校验 skill 结构
skilllite info <SKILL_DIR>      # 查看 skill 元信息
```

## Basic Code Template

```python
from skilllite import run_skill

# Execute a skill directly (CLI: skilllite list, skilllite run)
result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

> 如需 LangChain 集成，请使用 `pip install langchain-skilllite`，参见 [04. LangChain Integration](../04_langchain_integration/README.md)。

## Next Steps

- [03. Agentic Loop](../03_agentic_loop/README.md)
- [04. LangChain Integration](../04_langchain_integration/README.md)
