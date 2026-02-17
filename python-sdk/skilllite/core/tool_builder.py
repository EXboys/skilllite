"""
Tool Builder - Tool definition generation from skills.

Phase 4.12: Primary path uses input_schema from skilllite list --json (Rust API).
Fallback: flexible schema when API unavailable.
"""

import ast
from pathlib import Path
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .skill_info import SkillInfo
from .tools import ToolDefinition

if TYPE_CHECKING:
    from .registry import SkillRegistry


class ToolBuilder:
    """
    Builder for creating tool definitions from skills.
    
    Uses progressive disclosure - tool definitions only contain name and description,
    full SKILL.md content is injected when the tool is actually called.
    Schema comes from skilllite list --json when available.
    """
    
    def __init__(self, registry: "SkillRegistry"):
        """
        Initialize the tool builder.
        
        Args:
            registry: Skill registry for accessing skill info
        """
        self._registry = registry
    
    def get_tool_definitions(self, include_prompt_only: bool = False) -> List[ToolDefinition]:
        """
        Get tool definitions for registered skills.
        
        Includes:
        - Regular skills with a single entry_point
        - Multi-script tools (each script as a separate tool)
        
        Args:
            include_prompt_only: Whether to include prompt-only skills
            
        Returns:
            List of tool definitions
        """
        # Lazily analyze all skills for multi-script tools
        self._registry.analyze_all_multi_script_skills()
        
        definitions = []
        multi_script_skill_names = set(
            t["skill_name"] for t in self._registry.multi_script_tools.values()
        )
        
        # Add regular skills with single entry_point
        for info in self._registry.list_skills():
            if info.metadata.entry_point:
                definition = self._create_tool_definition(info)
                definitions.append(definition)
            elif info.is_bash_tool_skill:
                # Bash-tool skill: generate a tool accepting a 'command' string
                definition = self._create_bash_tool_definition(info)
                definitions.append(definition)
            elif info.name in multi_script_skill_names:
                # Skip - will be handled by multi-script tools below
                pass
            elif include_prompt_only:
                definition = self._create_tool_definition(info)
                definitions.append(definition)
        
        # Add multi-script tools
        for tool_name, tool_info in self._registry.multi_script_tools.items():
            skill_info = self._registry.get_skill(tool_info["skill_name"])
            if skill_info:
                definition = self._create_multi_script_tool_definition(
                    tool_name, tool_info, skill_info
                )
                definitions.append(definition)
        
        return definitions
    
    def get_tools_openai(self) -> List[Dict[str, Any]]:
        """Get tool definitions in OpenAI-compatible format."""
        return [d.to_openai_format() for d in self.get_tool_definitions()]
    
    def get_tools_claude_native(self) -> List[Dict[str, Any]]:
        """Get tool definitions in Claude's native API format."""
        return [d.to_claude_format() for d in self.get_tool_definitions()]
    
    def _create_tool_definition(self, info: SkillInfo) -> ToolDefinition:
        """Create a tool definition from skill info.
        
        Uses progressive disclosure:
        1. Tool definition only contains name and description (from YAML front matter)
        2. Uses a flexible schema that accepts any parameters
        3. Full SKILL.md content is injected when the tool is actually called,
           letting the LLM understand the expected parameters from the documentation
        """
        description = info.description or f"Execute the {info.name} skill"
        
        # Use a flexible schema that accepts any parameters
        # The LLM will understand the expected format from the full SKILL.md
        # content that is injected when the tool is called
        input_schema = {
            "type": "object",
            "properties": {},
            "additionalProperties": True
        }
        
        return ToolDefinition(
            name=info.name,
            description=description,
            input_schema=input_schema
        )
    
    def _create_bash_tool_definition(self, info: SkillInfo) -> ToolDefinition:
        """Create a tool definition for a bash-tool skill.

        Bash-tool skills use ``allowed-tools: Bash(prefix:*)`` and have no script
        entry point.  The LLM sends a ``command`` string which is validated and
        executed by ``skillbox bash``.
        """
        description = info.description or f"Execute {info.name} CLI commands"
        # Append allowed patterns hint so the LLM knows which commands are valid
        patterns = info.get_bash_patterns()
        if patterns:
            prefixes = ", ".join(p.command_prefix for p in patterns)
            description += f" (allowed commands: {prefixes})"

        input_schema = {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": f"The bash command to execute (must start with one of: {prefixes})"
                    if patterns
                    else "The bash command to execute",
                }
            },
            "required": ["command"],
        }

        return ToolDefinition(
            name=info.name,
            description=description,
            input_schema=input_schema,
        )

    def _create_multi_script_tool_definition(
        self,
        tool_name: str,
        tool_info: Dict[str, Any],
        skill_info: SkillInfo
    ) -> ToolDefinition:
        """Create a tool definition for a multi-script tool."""
        script_name = tool_info["script_name"]
        script_path = skill_info.path / tool_info["script_path"]
        description = f"Execute {script_name} from {skill_info.name} skill"
        
        if script_path.exists():
            try:
                script_content = script_path.read_text(encoding="utf-8")
                docstring = self._extract_script_docstring(script_content)
                if docstring:
                    first_line = docstring.strip().split('\n')[0].strip()
                    if first_line:
                        description = first_line
                        # Add action hint for common operations
                        if "init" in script_name.lower() or "create" in script_name.lower():
                            description += ". Call this tool directly to create a new skill."
                        elif "package" in script_name.lower():
                            description += ". Call this tool directly to package a skill."
                        elif "validate" in script_name.lower():
                            description += ". Call this tool directly to validate a skill."
            except Exception:
                pass
        
        input_schema = self._get_script_schema(tool_info)
        
        return ToolDefinition(
            name=tool_name,
            description=description,
            input_schema=input_schema
        )
    
    def _get_script_schema(self, tool_info: Dict[str, Any]) -> Dict[str, Any]:
        """Get input schema for a multi-script tool.

        Phase 4.12: Uses input_schema from skilllite list --json when available.
        Fallback: flexible schema when API unavailable.
        """
        schema = tool_info.get("input_schema")
        if isinstance(schema, dict):
            return schema
        return {
            "type": "object",
            "properties": {
                "args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Command line arguments to pass to the script"
                }
            },
            "required": []
        }
    
    def _extract_script_docstring(self, script_content: str) -> Optional[str]:
        """Extract the module-level docstring from a Python script."""
        try:
            tree = ast.parse(script_content)
            return ast.get_docstring(tree)
        except Exception:
            return None
    
