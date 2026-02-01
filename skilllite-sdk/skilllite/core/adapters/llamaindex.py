"""
LlamaIndex adapter for SkillLite.

Provides SkillLiteToolSpec for integrating SkillLite skills into LlamaIndex agents.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
    
    manager = SkillManager(skills_dir="./skills")
    tool_spec = SkillLiteToolSpec.from_manager(manager)
    tools = tool_spec.to_tool_list()
    
    # Use with LlamaIndex agent
    from llama_index.core.agent import ReActAgent
    agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

Requirements:
    pip install skilllite[llamaindex]
"""

from typing import Any, Dict, List, Optional, TYPE_CHECKING

try:
    from llama_index.core.tools import FunctionTool, ToolMetadata
    from llama_index.core.tools.types import BaseTool as LlamaBaseTool
except ImportError as e:
    raise ImportError(
        "LlamaIndex adapter requires llama-index. "
        "Install with: pip install skilllite[llamaindex]"
    ) from e

if TYPE_CHECKING:
    from ..manager import SkillManager


class SkillLiteToolSpec:
    """
    LlamaIndex ToolSpec for SkillLite.
    
    Provides a way to create LlamaIndex tools from SkillLite skills.
    
    Usage:
        manager = SkillManager(skills_dir="./skills")
        tool_spec = SkillLiteToolSpec.from_manager(manager)
        tools = tool_spec.to_tool_list()
        
        # Use with ReActAgent
        agent = ReActAgent.from_tools(tools, llm=llm)
        response = agent.chat("Your query")
    """
    
    def __init__(
        self,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None
    ):
        """
        Initialize SkillLiteToolSpec.
        
        Args:
            manager: SkillManager instance with registered skills
            skill_names: Optional list of skill names to include (default: all)
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
        """
        self.manager = manager
        self.skill_names = skill_names
        self.allow_network = allow_network
        self.timeout = timeout
    
    @classmethod
    def from_manager(
        cls,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None
    ) -> "SkillLiteToolSpec":
        """
        Create a SkillLiteToolSpec from a SkillManager.
        
        Args:
            manager: SkillManager instance
            skill_names: Optional list of skill names to include
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            
        Returns:
            SkillLiteToolSpec instance
        """
        return cls(
            manager=manager,
            skill_names=skill_names,
            allow_network=allow_network,
            timeout=timeout
        )
    
    def _create_skill_function(self, skill_name: str):
        """Create a callable function for a skill."""
        def skill_fn(**kwargs) -> str:
            try:
                result = self.manager.execute(
                    skill_name,
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
        
        return skill_fn
    
    def to_tool_list(self) -> List[LlamaBaseTool]:
        """
        Convert all skills to a list of LlamaIndex tools.
        
        Returns:
            List of FunctionTool instances
        """
        tools = []
        
        # Get executable skills
        skills = self.manager.list_executable_skills()
        
        for skill in skills:
            # Filter by name if specified
            if self.skill_names and skill.name not in self.skill_names:
                continue
            
            # Create function for this skill
            fn = self._create_skill_function(skill.name)
            
            # Create tool metadata
            metadata = ToolMetadata(
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill"
            )
            
            # Create FunctionTool
            tool = FunctionTool.from_defaults(
                fn=fn,
                name=skill.name,
                description=skill.description or f"Execute the {skill.name} skill"
            )
            tools.append(tool)
        
        return tools


__all__ = ["SkillLiteToolSpec"]

