# 03. Agentic Loop

## Core Examples

### basic_loop.py
Complete agent loop example

Includes two implementations:
1. **Manual Agent Loop** - No framework dependency, uses OpenAI API directly
2. **SkillLite Agent** - Uses SkillLite built-in Agent (recommended)

```bash
python basic_loop.py
```

## Basic Code Template

### Simplest Approach (Recommended)
```python
from skilllite import SkillRunner

runner = SkillRunner()
result = runner.run("Your request")
```

### Manual Agent Loop Implementation
```python
from skilllite import SkillManager
from openai import OpenAI

manager = SkillManager(skills_dir="./skills")
client = OpenAI()

tools = manager.get_tools()

# Message history
messages = [{"role": "user", "content": "Your request"}]

# Loop: LLM → Tool Call → Execute → Continue
for _ in range(max_iterations):
    response = client.chat.completions.create(
        model="gpt-4",
        messages=messages,
        tools=[t.to_openai_format() for t in tools]
    )

    # Handle tool calls
    if response.choices[0].message.tool_calls:
        for tool_call in response.choices[0].message.tool_calls:
            result = manager.execute(
                tool_call.function.name,
                json.loads(tool_call.function.arguments)
            )
            # Add result to message history
```

## Next Steps

- [04. LangChain Integration](../04_langchain_integration/README.md)
- [05. LlamaIndex Integration](../05_llamaindex_integration/README.md)
