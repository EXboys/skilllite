"""
LangChain Practical Example: Building an Agent with SkillLite

Prerequisites:
  pip install langchain langchain-openai
  Configure .env file

Usage:
  from langchain.agents import create_openai_tools_agent, AgentExecutor
  from langchain.prompts import ChatPromptTemplate

  # 1. Prepare tools
  tools = manager.get_tools()

  # 2. Create agent
  agent = create_openai_tools_agent(llm, tools, prompt)

  # 3. Execute
  executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
  result = executor.invoke({"input": "Your request"})
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager
from openai import OpenAI

# ========== Initialize ==========
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))
client = OpenAI()  # Automatically reads OPENAI_API_KEY

# ========== Approach 1: Manual Agent Loop (No Dependencies) ==========

def simple_agent(user_request: str, max_iterations: int = 5):
    """
    Simple Agent implementation without LangChain dependency

    Args:
        user_request: User request
        max_iterations: Maximum iterations

    Returns:
        Final response
    """
    tools = manager.get_tools()
    tools_json = [t.to_openai_format() for t in tools]

    messages = [
        {"role": "user", "content": user_request}
    ]

    for _ in range(max_iterations):
        # Call LLM
        response = client.chat.completions.create(
            model="gpt-4",
            messages=messages,
            tools=tools_json,
        )

        # Check if tool call is needed
        if response.choices[0].message.tool_calls:
            # Handle tool calls
            for tool_call in response.choices[0].message.tool_calls:
                skill_name = tool_call.function.name
                import json
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


# ========== Approach 2: Using LangChain (Recommended for Production) ==========

def langchain_agent(user_request: str):
    """
    Using LangChain Agent (requires langchain installation)
    """
    try:
        from langchain.agents import create_openai_tools_agent, AgentExecutor
        from langchain_openai import ChatOpenAI
        from langchain.prompts import ChatPromptTemplate

        # Prepare tools
        tools = manager.get_tools()

        # Initialize LLM
        llm = ChatOpenAI(model="gpt-4")

        # Create Agent
        prompt = ChatPromptTemplate.from_messages([
            ("system", "You are a helpful assistant that can use the provided tools to help users."),
            ("human", "{input}"),
            ("placeholder", "{agent_scratchpad}")
        ])

        agent = create_openai_tools_agent(llm, tools, prompt)

        # Execute Agent
        executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
        result = executor.invoke({"input": user_request})

        return result["output"]
    except ImportError:
        print("❌ Please install LangChain first: pip install langchain langchain-openai")
        return None


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me analyze today's weather"

    # Using Approach 1 (no dependencies)
    print("Using simple Agent loop...")
    # result = simple_agent(request)
    # print(result)

    # Using Approach 2 (LangChain)
    print("Using LangChain Agent...")
    # result = langchain_agent(request)
    # print(result)
