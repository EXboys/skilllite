"""
LangChain adapter for SkillLite.

Provides SkillLiteTool and SkillLiteToolkit for integrating SkillLite
skills into LangChain agents.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.langchain import SkillLiteToolkit
    
    manager = SkillManager(skills_dir="./skills")
    tools = SkillLiteToolkit.from_manager(manager)
    
    # Use with LangChain agent
    from langchain.agents import create_openai_tools_agent, AgentExecutor
    agent = create_openai_tools_agent(llm, tools, prompt)
    executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)

Requirements:
    pip install skilllite[langchain]
"""

from typing import Any, Dict, List, Optional, Type, TYPE_CHECKING
import asyncio

try:
    from langchain_core.tools import BaseTool
    from langchain_core.callbacks import CallbackManagerForToolRun, AsyncCallbackManagerForToolRun
    from pydantic import BaseModel, Field
except ImportError as e:
    raise ImportError(
        "LangChain adapter requires langchain-core. "
        "Install with: pip install skilllite[langchain]"
    ) from e

if TYPE_CHECKING:
    from ..manager import SkillManager


class SkillLiteTool(BaseTool):
    """
    LangChain BaseTool adapter for a single SkillLite skill.
    
    This wraps a SkillLite skill as a LangChain tool, enabling it to be
    used with LangChain agents.
    
    Attributes:
        name: Tool name (same as skill name)
        description: Tool description
        manager: SkillManager instance
        skill_name: Name of the skill to execute
        allow_network: Whether to allow network access
        timeout: Execution timeout in seconds
    """
    
    name: str = Field(description="Tool name")
    description: str = Field(description="Tool description")
    args_schema: Optional[Type[BaseModel]] = Field(default=None, description="Pydantic schema for arguments")
    
    # SkillLite specific fields
    manager: Any = Field(exclude=True)  # SkillManager instance
    skill_name: str = Field(description="SkillLite skill name")
    allow_network: bool = Field(default=False, description="Allow network access")
    timeout: Optional[int] = Field(default=None, description="Execution timeout in seconds")
    
    class Config:
        arbitrary_types_allowed = True
    
    def _run(
        self,
        run_manager: Optional[CallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """Execute the skill synchronously."""
        try:
            result = self.manager.execute(
                self.skill_name,
                kwargs,
                allow_network=self.allow_network,
                timeout=self.timeout
            )
            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"
    
    async def _arun(
        self,
        run_manager: Optional[AsyncCallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """Execute the skill asynchronously."""
        # Run in thread pool to avoid blocking
        return await asyncio.to_thread(self._run, run_manager, **kwargs)


class SkillLiteToolkit:
    """
    LangChain Toolkit for SkillLite.
    
    Provides a convenient way to create LangChain tools from all skills
    registered in a SkillManager.
    
    Usage:
        manager = SkillManager(skills_dir="./skills")
        tools = SkillLiteToolkit.from_manager(manager)
        
        # Or with options
        tools = SkillLiteToolkit.from_manager(
            manager,
            skill_names=["calculator", "web_search"],  # Only specific skills
            allow_network=True,
            timeout=60
        )
    """
    
    @staticmethod
    def from_manager(
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None
    ) -> List[SkillLiteTool]:
        """
        Create LangChain tools from a SkillManager.
        
        Args:
            manager: SkillManager instance with registered skills
            skill_names: Optional list of skill names to include (default: all)
            allow_network: Whether to allow network access for all tools
            timeout: Execution timeout in seconds for all tools
            
        Returns:
            List of SkillLiteTool instances
        """
        tools = []
        
        # Get executable skills
        skills = manager.list_executable_skills()
        
        for skill in skills:
            # Filter by name if specified
            if skill_names and skill.name not in skill_names:
                continue
            
            # Create tool
            tool = SkillLiteTool(
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill",
                manager=manager,
                skill_name=skill.name,
                allow_network=allow_network,
                timeout=timeout
            )
            tools.append(tool)
        
        return tools


__all__ = ["SkillLiteTool", "SkillLiteToolkit"]

