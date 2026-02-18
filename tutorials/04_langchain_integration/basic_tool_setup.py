"""
LangChain Integration: Using SkillLite as tools

Prerequisites:
  pip install langchain-skilllite

Example: Convert SkillLite skills to LangChain tools
"""

from pathlib import Path

skills_dir = str(Path(__file__).parent / "../../.skills")

# ========== Using SkillLiteToolkit.from_directory ==========

try:
    from langchain_skilllite import SkillLiteToolkit

    # Create LangChain tools from skills directory
    tools = SkillLiteToolkit.from_directory(skills_dir)

    # Or with options:
    # tools = SkillLiteToolkit.from_directory(
    #     skills_dir,
    #     skill_names=["calculator", "web_search"],
    #     allow_network=True,
    #     timeout=60,
    # )

    print(f"✅ Created {len(tools)} LangChain tools using SkillLiteToolkit.from_directory")

    for tool in tools:
        print(f"   - {tool.name}: {tool.description}")

except ImportError:
    print("❌ langchain-skilllite not installed. Install with: pip install langchain-skilllite")
    tools = []
