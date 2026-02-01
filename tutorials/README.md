# SkillLite Tutorials & Examples

Complete SkillLite usage examples to quickly get started with all core features and framework integrations.

## 📚 Tutorial Directory

| Tutorial | Time | Difficulty | Content |
|----------|------|------------|---------|
| [01. Basic Usage](./01_basic) | 5 min | ⭐ | Simplest examples, one-line execution |
| [02. Skill Management](./02_skill_management) | 10 min | ⭐⭐ | Create and manage skills |
| [03. Agentic Loop](./03_agentic_loop) | 15 min | ⭐⭐ | Multi-turn conversations and tool calls |
| [04. LangChain Integration](./04_langchain_integration) | 15 min | ⭐⭐⭐ | Integration with LangChain framework |
| [05. LlamaIndex Integration](./05_llamaindex_integration) | 15 min | ⭐⭐⭐ | RAG + skill execution |
| [06. MCP Server](./06_mcp_server) | 10 min | ⭐⭐ | Claude Desktop integration |
| [07. OpenCode Integration](./07_opencode_integration) | 10 min | ⭐⭐ | OpenCode AI coding agent integration |

## 🚀 Quick Start

### 1. Environment Setup

```bash
# Install SkillLite
pip install skilllite

# Install skillbox sandbox
skilllite install

# Create .env configuration file
cp .env.example .env
# Edit .env and add your API key
```

### 2. Run Your First Example

```bash
cd tutorials/01_basic
python hello_world.py
```

### 3. Choose Your Learning Path

#### Beginners
1. [01. Basic Usage](./01_basic/README.md)
2. [02. Skill Management](./02_skill_management/README.md)
3. [03. Agentic Loop](./03_agentic_loop/README.md)

#### Using LangChain
1. [01. Basic Usage](./01_basic/README.md)
2. [04. LangChain Integration](./04_langchain_integration/README.md)

#### Using LlamaIndex
1. [01. Basic Usage](./01_basic/README.md)
2. [05. LlamaIndex Integration](./05_llamaindex_integration/README.md)

#### Using Claude Desktop
1. [01. Basic Usage](./01_basic/README.md)
2. [06. MCP Server](./06_mcp_server/README.md)

#### Using OpenCode
1. [01. Basic Usage](./01_basic/README.md)
2. [07. OpenCode Integration](./07_opencode_integration/README.md)

## 📖 Code Examples (Quick Reference)

### Simplest Usage
```python
from skilllite import SkillRunner

runner = SkillRunner()
result = runner.run("Help me with this task")
print(result)
```

### Direct Skill Execution (No LLM)
```python
from skilllite import SkillManager

manager = SkillManager(skills_dir="./skills")
result = manager.execute("skill_name", {"param": "value"})
```

### LangChain Integration
```python
from skilllite import SkillManager
from langchain.agents import create_openai_tools_agent, AgentExecutor

manager = SkillManager(skills_dir="./skills")
tools = manager.get_tools()

# Create agent and execute
agent = create_openai_tools_agent(llm, tools, prompt)
executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
result = executor.invoke({"input": "Your request"})
```

### LlamaIndex Integration
```python
from skilllite import SkillManager
from llama_index.core.agent import ReActAgent

manager = SkillManager(skills_dir="./skills")
tools = manager.get_tools()

agent = ReActAgent.from_tools(
    tools=[t.to_openai_format() for t in tools],
    llm=llm
)
response = agent.chat("Your query")
```

## 🔧 All Example Files

### 01_basic (Basic)
- `hello_world.py` - One-line execution
- `direct_execution.py` - Direct skill execution
- `tool_definitions.py` - View tool formats

### 02_skill_management (Skill Management)
- `list_skills.py` - List all skills
- `execute_skill.py` - Execute skills

### 03_agentic_loop (Agentic Loop)
- `basic_loop.py` - Basic agent loop

### 04_langchain_integration (LangChain)
- `basic_tool_setup.py` - Tool setup
- `agent_example.py` - Agent examples (2 approaches)

### 05_llamaindex_integration (LlamaIndex)
- `basic_usage.py` - RAG examples (3 approaches)

### 06_mcp_server (MCP Server)
- `mcp_client_test.py` - MCP client test

### 07_opencode_integration (OpenCode)
- `verify_setup.py` - Verify OpenCode + SkillLite setup

## ⚙️ Environment Configuration

Create a `.env` file:

```env
# API Configuration
API_KEY=sk-...                                    # Your API key
BASE_URL=https://api.openai.com/v1              # API endpoint
MODEL=gpt-4                                      # Model name

# Or use Anthropic Claude
ANTHROPIC_API_KEY=sk-ant-...

# SkillLite Configuration
SKILLLITE_SKILLS_DIR=./skills                   # Skills directory
SKILLBOX_SANDBOX_LEVEL=3                        # Sandbox level (1/2/3)
```

## ❓ FAQ

**Q: Do I need to install Rust?**
A: No, `skilllite install` will automatically download pre-compiled binaries.

**Q: Can I execute skills without an LLM?**
A: Yes, see [01_basic/direct_execution.py](./01_basic/direct_execution.py)

**Q: How do I use it with LangChain?**
A: See [04_langchain_integration](./04_langchain_integration/README.md)

**Q: Which LLMs are supported?**
A: All OpenAI-compatible APIs + Claude native API

## 📚 More Resources

- [SkillLite SDK Documentation](../skilllite-sdk/README.md)
- [Framework Integration Analysis](../docs/Framework_Integration_Analysis.md)
- [GitHub Repository](https://github.com/EXboys/skilllite)

