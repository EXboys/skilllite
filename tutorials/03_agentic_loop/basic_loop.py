"""
Agentic Loop: Multi-turn conversations and tool calls

Quick Start:
1. python basic_loop.py - Basic agent loop (no framework dependency)
2. python claude_native.py - Using Claude native API
"""

import sys
import os
import json
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager
from openai import OpenAI

# Initialize
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))
client = OpenAI()  # Automatically reads OPENAI_API_KEY

# ========== Approach 1: Manual Agent Loop ==========

def basic_agent_loop(user_request: str, max_iterations: int = 5):
    """
    Simple agent loop implementation
    """
    tools = manager.get_tools()
    tools_json = [t.to_openai_format() for t in tools]

    messages = [
        {"role": "user", "content": user_request}
    ]

    for iteration in range(max_iterations):
        print(f"\nIteration {iteration + 1}...")

        # Call LLM
        response = client.chat.completions.create(
            model="gpt-4",
            messages=messages,
            tools=tools_json,
        )

        # If LLM called a tool
        if response.choices[0].message.tool_calls:
            for tool_call in response.choices[0].message.tool_calls:
                skill_name = tool_call.function.name
                args = json.loads(tool_call.function.arguments)

                # Execute skill
                result = manager.execute(skill_name, args)

                # Add to message history
                messages.append({
                    "role": "assistant",
                    "content": response.choices[0].message.content
                })
                messages.append({
                    "role": "user",
                    "content": result.output if result.success else f"Error: {result.error}"
                })
        else:
            # No tool call, return final answer
            return response.choices[0].message.content

    return "Maximum iterations reached"


# ========== Approach 2: Using SkillLite Built-in Agent ==========

def skilllite_agent(user_request: str):
    """
    Using SkillLite built-in Agent (recommended)
    """
    from skilllite import SkillRunner

    runner = SkillRunner()
    return runner.run(user_request)


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me complete a task"

    # Using SkillLite built-in Agent (recommended)
    result = skilllite_agent(request)
    print(f"Final result: {result}")

    # Or use manual agent loop
    # result = basic_agent_loop(request)
    # print(f"Final result: {result}")
