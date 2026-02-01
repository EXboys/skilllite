# 05. LlamaIndex Integration

SkillLite provides official LlamaIndex adapters through `SkillLiteToolSpec`.

## Prerequisites

```bash
pip install skilllite[llamaindex] llama-index-llms-openai
```

## Quick Start (Recommended)

```python
from skilllite import SkillManager
from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
from llama_index.core.agent import ReActAgent
from llama_index.llms.openai import OpenAI

# 1. Create LlamaIndex tools from SkillManager
manager = SkillManager(skills_dir="./skills")
tool_spec = SkillLiteToolSpec.from_manager(manager)
tools = tool_spec.to_tool_list()

# 2. Create Agent
llm = OpenAI(model="gpt-4")
agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

# 3. Execute
response = agent.chat("Your query")
```

## Advanced Options

```python
# Filter specific skills and configure options
tool_spec = SkillLiteToolSpec.from_manager(
    manager,
    skill_names=["calculator", "web_search"],  # Only specific skills
    allow_network=True,                         # Allow network access
    timeout=60                                  # Execution timeout in seconds
)
tools = tool_spec.to_tool_list()
```

## Examples

### basic_usage.py
LlamaIndex Agent + SkillLite skill execution

Includes three implementation approaches:
- Approach 1: SkillLiteToolSpec (recommended)
- Approach 2: With custom options
- Approach 3: RAG + Skills pipeline

```bash
python basic_usage.py
```

## API Reference

### SkillLiteToolSpec

Factory class to create LlamaIndex tools from SkillManager.

```python
tool_spec = SkillLiteToolSpec.from_manager(
    manager: SkillManager,
    skill_names: Optional[List[str]] = None,  # Filter skills
    allow_network: bool = False,               # Network access
    timeout: Optional[int] = None              # Timeout in seconds
)

# Convert to LlamaIndex tools
tools = tool_spec.to_tool_list()  # Returns List[FunctionTool]
```

## Use Cases

1. **RAG + Tool Execution**: Retrieve documents + execute data processing skills
2. **Data Analysis**: Extract information from documents + execute analysis skills
3. **Multi-step Workflows**: Complex tasks combining retrieval and execution
4. **ReAct Agents**: Reasoning + Acting with SkillLite skills

## Next Steps

- [04. LangChain Integration](../04_langchain_integration/README.md) (for comparison)
- [06. MCP Server](../06_mcp_server/README.md)
