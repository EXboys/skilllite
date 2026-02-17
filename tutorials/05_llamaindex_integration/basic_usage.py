"""
LlamaIndex Practical Example: Using SkillLite with LlamaIndex Agents

Prerequisites:
  pip install skilllite[llamaindex] llama-index-llms-openai

Usage with SkillLiteToolSpec (Recommended):
  from skilllite import SkillManager
  from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
  from llama_index.core.agent import ReActAgent
  from llama_index.llms.openai import OpenAI

  manager = SkillManager(skills_dir="./skills")
  tool_spec = SkillLiteToolSpec.from_manager(manager)
  tools = tool_spec.to_tool_list()

  llm = OpenAI(model="gpt-4")
  agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)
  response = agent.chat("Your query")
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../python-sdk'))

from skilllite import SkillManager

# ========== Initialize ==========
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))


# ========== Approach 1: Using SkillLiteToolSpec (Recommended) ==========

def llamaindex_agent_with_toolspec(query: str):
    """
    Using LlamaIndex Agent with SkillLiteToolSpec (recommended approach)
    """
    try:
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
        from llama_index.core.agent import ReActAgent
        from llama_index.llms.openai import OpenAI as LlamaOpenAI

        # Create LlamaIndex tools using SkillLiteToolSpec
        tool_spec = SkillLiteToolSpec.from_manager(manager)
        tools = tool_spec.to_tool_list()

        print(f"✅ Created {len(tools)} LlamaIndex tools")
        for tool in tools:
            print(f"   - {tool.metadata.name}: {tool.metadata.description}")

        # Initialize LLM
        llm = LlamaOpenAI(model="gpt-4")

        # Create Agent
        agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

        # Execute
        response = agent.chat(query)
        return str(response)
    except ImportError as e:
        print(f"❌ Please install dependencies: pip install skilllite[llamaindex] llama-index-llms-openai")
        print(f"   Error: {e}")
        return None


# ========== Approach 2: With Options ==========

def llamaindex_agent_with_options(query: str):
    """
    Using SkillLiteToolSpec with custom options
    """
    try:
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
        from llama_index.core.agent import ReActAgent
        from llama_index.llms.openai import OpenAI as LlamaOpenAI

        # Create tools with options
        tool_spec = SkillLiteToolSpec.from_manager(
            manager,
            skill_names=["calculator"],  # Only specific skills
            allow_network=True,           # Allow network access
            timeout=60                    # Execution timeout
        )
        tools = tool_spec.to_tool_list()

        llm = LlamaOpenAI(model="gpt-4")
        agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

        response = agent.chat(query)
        return str(response)
    except ImportError as e:
        print(f"❌ Missing dependencies: {e}")
        return None


# ========== Approach 3: RAG + Skills Pipeline ==========

def rag_with_skills(documents: list, query: str):
    """
    Complete RAG + skill execution pipeline
    """
    try:
        from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
        from llama_index.core import VectorStoreIndex
        from llama_index.core.agent import ReActAgent
        from llama_index.llms.openai import OpenAI as LlamaOpenAI

        # 1. Build RAG index
        index = VectorStoreIndex.from_documents(documents)
        query_engine = index.as_query_engine()

        # 2. Create skill tools
        tool_spec = SkillLiteToolSpec.from_manager(manager)
        skill_tools = tool_spec.to_tool_list()

        # 3. Create agent with both RAG and skills
        llm = LlamaOpenAI(model="gpt-4")
        agent = ReActAgent.from_tools(
            skill_tools,  # Add skill tools
            llm=llm,
            verbose=True
        )

        # 4. Execute query
        response = agent.chat(query)
        return str(response)
    except ImportError as e:
        print(f"❌ Missing dependencies: {e}")
        return None


# ========== Test ==========

if __name__ == "__main__":
    print("Available approaches:")
    print("1. llamaindex_agent_with_toolspec() - SkillLiteToolSpec (recommended)")
    print("2. llamaindex_agent_with_options() - With custom options")
    print("3. rag_with_skills() - RAG + Skills pipeline")
    print()

    # Uncomment to test:
    # result = llamaindex_agent_with_toolspec("Calculate 15 * 23")
    # print(result)
