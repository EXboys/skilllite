# 04. LangChain Integration

SkillLite provides official LangChain adapters through `SkillLiteToolkit` and `SkillLiteTool`.

## Prerequisites

```bash
pip install skilllite[langchain] langchain-openai
```

## Quick Start (Recommended)

```python
from skilllite import SkillManager
from skilllite.core.adapters.langchain import SkillLiteToolkit
from langchain.agents import create_openai_tools_agent, AgentExecutor
from langchain_openai import ChatOpenAI
from langchain.prompts import ChatPromptTemplate

# 1. Create LangChain tools from SkillManager
manager = SkillManager(skills_dir="./skills")
tools = SkillLiteToolkit.from_manager(manager)

# 2. Create Agent
llm = ChatOpenAI(model="gpt-4")
prompt = ChatPromptTemplate.from_messages([
    ("system", "You are a helpful assistant."),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}")
])
agent = create_openai_tools_agent(llm, tools, prompt)

# 3. Execute
executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
result = executor.invoke({"input": "Your request"})
```

## Advanced Options

```python
# Filter specific skills and configure options
tools = SkillLiteToolkit.from_manager(
    manager,
    skill_names=["calculator", "web_search"],  # Only specific skills
    allow_network=True,                         # Allow network access
    timeout=60                                  # Execution timeout in seconds
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

Factory class to create LangChain tools from SkillManager.

```python
SkillLiteToolkit.from_manager(
    manager: SkillManager,
    skill_names: Optional[List[str]] = None,  # Filter skills
    allow_network: bool = False,               # Network access
    timeout: Optional[int] = None              # Timeout in seconds
) -> List[SkillLiteTool]
```

### SkillLiteTool

LangChain BaseTool wrapper for a single SkillLite skill.

```python
tool = SkillLiteTool(
    name="skill_name",
    description="Skill description",
    manager=manager,
    skill_name="skill_name",
    allow_network=False,
    timeout=None
)
```

## Next Steps

- [05. LlamaIndex Integration](../05_llamaindex_integration/README.md)
- [06. MCP Server](../06_mcp_server/README.md)
