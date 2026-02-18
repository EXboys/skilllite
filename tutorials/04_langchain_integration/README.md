# 04. LangChain Integration

SkillLite provides LangChain integration through the [langchain-skilllite](https://pypi.org/project/langchain-skilllite/) package.

## Prerequisites

```bash
pip install langchain-skilllite langchain-openai
```

## Quick Start (Recommended)

```python
from langchain_skilllite import SkillLiteToolkit
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import create_react_agent

# 1. Create LangChain tools from skills directory
tools = SkillLiteToolkit.from_directory("./skills")

# 2. Create Agent
llm = ChatOpenAI(model="gpt-4")
agent = create_react_agent(llm, tools)

# 3. Execute
result = agent.invoke({"messages": [("user", "Your request")]})
```

## Advanced Options

```python
# Filter specific skills and configure options
tools = SkillLiteToolkit.from_directory(
    "./skills",
    skill_names=["calculator", "web_search"],  # Only specific skills
    allow_network=True,                         # Allow network access
    timeout=60,                                 # Execution timeout in seconds
    sandbox_level=3,                            # 1/2/3
    confirmation_callback=confirm_execution    # For sandbox_level=3
)
```

## Examples

### basic_tool_setup.py
Convert SkillLite skills to LangChain tools
```bash
python basic_tool_setup.py
```

### agent_example.py
Build a LangChain Agent using SkillLite skills

Includes three implementation approaches:
- Approach 1: Manual agent loop (no dependencies)
- Approach 2: SkillLiteToolkit (recommended)
- Approach 3: Built-in AgenticLoop

## API Reference

### SkillLiteToolkit

Factory class to create LangChain tools from skills directory.

```python
SkillLiteToolkit.from_directory(
    skills_dir: str,
    skill_names: Optional[List[str]] = None,  # Filter skills
    allow_network: bool = False,              # Network access
    timeout: Optional[int] = None,            # Timeout in seconds
    sandbox_level: int = 3,                    # 1/2/3
    confirmation_callback: Optional[Callable] = None  # For sandbox_level=3
)
```

See [langchain-skilllite on PyPI](https://pypi.org/project/langchain-skilllite/) for full API documentation.

## Next Steps

- [05. LlamaIndex Integration](../05_llamaindex_integration/README.md)
- [06. MCP Server](../06_mcp_server/README.md)
