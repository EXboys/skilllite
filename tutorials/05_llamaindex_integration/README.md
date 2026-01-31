# 05. LlamaIndex Integration

## Core Examples

### basic_usage.py
LlamaIndex RAG + SkillLite skill execution

Includes three implementation approaches:
- Approach 1: Simple RAG + skill execution
- Approach 2: Using LlamaIndex Agent (recommended)
- Approach 3: Complete RAG pipeline

## Basic Code Template

```python
from skilllite import SkillManager
from llama_index.core.agent import ReActAgent
from llama_index.llms.openai import OpenAI

# Initialize
manager = SkillManager(skills_dir="./skills")
tools = manager.get_tools()

# Create Agent
llm = OpenAI(model="gpt-4")
agent = ReActAgent.from_tools(
    tools=[t.to_openai_format() for t in tools],
    llm=llm,
    verbose=True
)

# Execute
response = agent.chat("Your query")
```

## Prerequisites

```bash
pip install llama-index
```

## Use Cases

1. **RAG + Tool Execution**: Retrieve documents + execute data processing skills
2. **Data Analysis**: Extract information from documents + execute analysis skills
3. **Multi-step Workflows**: Complex tasks combining retrieval and execution

## Next Steps

- [04. LangChain Integration](../04_langchain_integration/README.md) (for comparison)
- [06. MCP Server](../06_mcp_server/README.md)
