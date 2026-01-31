# 04. LangChain Integration

## Core Examples

### basic_tool_setup.py
Convert SkillLite skills to LangChain tools
```bash
python basic_tool_setup.py
```

### agent_example.py
Build a LangChain Agent using SkillLite skills

Includes two implementation approaches:
- Approach 1: Manual agent loop implementation (no dependencies)
- Approach 2: Using LangChain (recommended for production)

## Basic Code Template

```python
from skilllite import SkillManager
from langchain.agents import create_openai_tools_agent, AgentExecutor
from langchain_openai import ChatOpenAI

# Initialize
manager = SkillManager(skills_dir="./skills")
tools = manager.get_tools()  # Get OpenAI format tools

# Create Agent
llm = ChatOpenAI(model="gpt-4")
agent = create_openai_tools_agent(llm, tools, prompt)
executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)

# Execute
result = executor.invoke({"input": "Your request"})
```

## Prerequisites

```bash
pip install langchain langchain-openai
```

## Next Steps

- [05. LlamaIndex Integration](../05_llamaindex_integration/README.md)
- [06. MCP Server](../06_mcp_server/README.md)
