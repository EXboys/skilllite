"""
LangChain Practical Example: Building an Agent with SkillLite

Prerequisites:
  pip install skilllite[langchain] langchain-openai
  Configure OPENAI_API_KEY environment variable

Usage (RPC-based, no SkillManager):
  from skilllite.core.adapters.langchain import SkillLiteToolkit
  from skilllite import SkillRunner

  tools = SkillLiteToolkit.from_skills_dir("./skills")
  # Or use SkillRunner for built-in agent (no LangChain)
  result = SkillRunner().run("Your request")
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

skills_dir = str(Path(__file__).parent / "../../.skills")

# ========== Approach 1: LangChain with SkillLiteToolkit.from_skills_dir ==========

def langchain_agent_with_toolkit(user_request: str):
    """Using LangChain Agent with SkillLiteToolkit (RPC-based)."""
    try:
        from skilllite.core.adapters.langchain import SkillLiteToolkit
        from langchain.agents import create_openai_tools_agent, AgentExecutor
        from langchain_openai import ChatOpenAI
        from langchain.prompts import ChatPromptTemplate

        tools = SkillLiteToolkit.from_skills_dir(skills_dir)
        llm = ChatOpenAI(model="gpt-4")

        prompt = ChatPromptTemplate.from_messages([
            ("system", "You are a helpful assistant that can use the provided tools to help users."),
            ("human", "{input}"),
            ("placeholder", "{agent_scratchpad}")
        ])

        agent = create_openai_tools_agent(llm, tools, prompt)
        executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)
        result = executor.invoke({"input": user_request})
        return result["output"]
    except ImportError as e:
        print(f"❌ pip install skilllite[langchain] langchain-openai")
        print(f"   Error: {e}")
        return None


# ========== Approach 2: SkillRunner (Built-in, No LangChain) ==========

def skilllite_agent(user_request: str):
    """Using SkillRunner (agent_chat RPC) — no LangChain."""
    from skilllite import SkillRunner
    return SkillRunner(skills_dir=skills_dir).run(user_request)


# ========== Test ==========

if __name__ == "__main__":
    request = "Help me analyze today's weather"

    print("Available approaches:")
    print("1. langchain_agent_with_toolkit() - LangChain with SkillLiteToolkit")
    print("2. skilllite_agent() - SkillRunner (built-in agent)")
    print()

    # Uncomment to test:
    # result = langchain_agent_with_toolkit(request)
    # result = skilllite_agent(request)
    # print(result)
