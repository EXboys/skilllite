"""
LlamaIndex Practical Example: RAG + SkillLite Skill Execution

Prerequisites:
  pip install llama-index

Workflow:
  1. LlamaIndex performs Retrieval Augmented Generation (RAG)
  2. Select appropriate skills based on context
  3. SkillLite safely executes skills
  4. Aggregate results and return to user
"""

import sys
import os
from pathlib import Path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../skilllite-sdk'))

from skilllite import SkillManager

# ========== Initialize ==========
skills_dir = Path(__file__).parent / "../../.skills"
manager = SkillManager(skills_dir=str(skills_dir))

# ========== Approach 1: Simple RAG + Skill Execution ==========

def rag_with_skills(query: str, documents: list = None):
    """
    Basic RAG + skill execution workflow

    Args:
        query: User query
        documents: List of documents

    Returns:
        Processing result
    """
    # 1. Get available skills
    tools = manager.get_tools()

    # 2. Add LlamaIndex RAG logic here
    # from llama_index.core import VectorStoreIndex
    # index = VectorStoreIndex.from_documents(documents)
    # context = index.query(query)

    # 3. Select appropriate skill based on query (simplified)
    if any(keyword in query for keyword in ["analyze", "data"]):
        # Use analysis skill
        pass

    return "Query processing complete"


# ========== Approach 2: Complete LlamaIndex Agent ==========

def llamaindex_agent(query: str):
    """
    Using LlamaIndex Agent (requires llama-index installation)
    """
    try:
        from llama_index.core.agent import ReActAgent
        from llama_index.llms.openai import OpenAI as LlamaOpenAI

        # Prepare tools (OpenAI format)
        tools = manager.get_tools()

        # Initialize LLM
        llm = LlamaOpenAI(model="gpt-4")

        # Create Agent
        agent = ReActAgent.from_tools(
            tools=[t.to_openai_format() for t in tools],
            llm=llm,
            verbose=True
        )

        # Execute
        response = agent.chat(query)
        return str(response)
    except ImportError:
        print("❌ Please install LlamaIndex first: pip install llama-index")
        return None


# ========== Approach 3: RAG Pipeline ==========

def rag_pipeline(documents: list, query: str):
    """
    Complete RAG pipeline example

    Args:
        documents: List of documents
        query: User query
    """
    try:
        from llama_index.core import VectorStoreIndex

        # 1. Build index
        index = VectorStoreIndex.from_documents(documents)

        # 2. Execute query
        retriever = index.as_retriever()
        nodes = retriever.retrieve(query)

        # 3. Select skills based on retrieval results
        tools = manager.get_tools()

        # 4. Execute relevant skills
        results = []
        for node in nodes:
            # Select skills based on node content here
            pass

        return results
    except ImportError:
        print("❌ Please install LlamaIndex first: pip install llama-index")
        return None


# ========== Test ==========

if __name__ == "__main__":
    # Simple RAG
    # result = rag_with_skills("Analyze this data")

    # LlamaIndex Agent
    # result = llamaindex_agent("Help me with this task")

    pass
