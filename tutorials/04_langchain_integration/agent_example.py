"""
LangChain Practical Example: Building an Agent with SkillLite

Prerequisites:
  pip install skilllite[langchain] langchain-openai
  Configure OPENAI_API_KEY environment variable

Usage with SkillLiteToolkit (Recommended):
  from skilllite import SkillManager
  from skilllite.core.adapters.langchain import SkillLiteToolkit
  from langchain.agents import create_openai_tools_agent, AgentExecutor
  from langchain_openai import ChatOpenAI

  # 1. Create tools from SkillManager
  manager = SkillManager(skills_dir="./skills")
  tools = SkillLiteToolkit.from_manager(manager)

  # 2. Create agent
  llm = ChatOpenAI(model="gpt-4")
  agent = create_openai_tools_agent(llm, tools, prompt)

  # 3. Execute
  executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
  result = executor.invoke({"input": "Your request"})
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillManager

# ========== Initialize ==========
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

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


# ========== Approach 2: Using SkillLiteToolkit (Recommended) ==========

def langchain_agent_with_toolkit(user_request: str):
    """
    Using LangChain Agent with SkillLiteToolkit (recommended approach)
    """
    try:
        from skilllite.core.adapters.langchain import SkillLiteToolkit
        from langchain.agents import create_openai_tools_agent, AgentExecutor
        from langchain_openai import ChatOpenAI
        from langchain.prompts import ChatPromptTemplate

        # Create LangChain tools using SkillLiteToolkit
        tools = SkillLiteToolkit.from_manager(manager)

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
    except ImportError as e:
        print(f"❌ Please install dependencies: pip install skilllite[langchain] langchain-openai")
        print(f"   Error: {e}")
        return None


# ========== Approach 3: Using AgenticLoop (Built-in, No LangChain) ==========

def agentic_loop_example(user_request: str):
    """
    Using SkillLite's built-in AgenticLoop (no LangChain dependency)
    """
    try:
        from openai import OpenAI

        client = OpenAI()  # Reads OPENAI_API_KEY from environment

        # Create agentic loop
        loop = manager.create_agentic_loop(
            client=client,
            model="gpt-4",
            system_prompt="You are a helpful assistant.",
            max_iterations=10,
            api_format="openai"
        )

        # Run the loop
        result = loop.run(user_request)
        return result
    except ImportError:
        print("❌ Please install openai: pip install openai")
        return None


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me analyze today's weather"

    print("Available approaches:")
    print("1. simple_agent() - Manual loop, no dependencies")
    print("2. langchain_agent_with_toolkit() - LangChain with SkillLiteToolkit")
    print("3. agentic_loop_example() - Built-in AgenticLoop")
    print()

    # Uncomment to test:
    # result = simple_agent(request)
    # result = langchain_agent_with_toolkit(request)
    # result = agentic_loop_example(request)
    # print(result)
