"""
Prompt Builder - System prompt context generation.

This module handles:
- Generating system prompt context for LLM
- Formatting skill information for different modes
- Skills status reporting and logging
"""

import json
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .skill_info import SkillInfo

if TYPE_CHECKING:
    from .registry import SkillRegistry


class PromptBuilder:
    """
    Builder for generating system prompt context from skills.
    
    Supports multiple disclosure modes:
    - summary: Brief overview of skills
    - standard: Input schema and usage summary
    - progressive: Summary with "more details available" hint
    - full: Complete instructions and references
    """
    
    def __init__(self, registry: "SkillRegistry"):
        """
        Initialize the prompt builder.
        
        Args:
            registry: Skill registry for accessing skill info
        """
        self._registry = registry
    
    def get_system_prompt_context(
        self,
        include_full_instructions: bool = True,
        include_references: bool = False,
        include_assets: bool = False,
        skills: Optional[List[str]] = None,
        mode: str = "full",
        max_tokens_per_skill: Optional[int] = None
    ) -> str:
        """
        Generate system prompt context containing skill information.
        
        Args:
            include_full_instructions: Include full instructions (affects mode)
            include_references: Include reference documents
            include_assets: Include asset files
            skills: Specific skills to include (None = all)
            mode: Disclosure mode (summary, standard, progressive, full)
            max_tokens_per_skill: Max tokens per skill content
            
        Returns:
            Formatted system prompt context string
        """
        if not include_full_instructions and mode == "full":
            mode = "standard"
        
        lines = ["# Available Skills\n"]
        target_skills = self._registry.list_skills()
        if skills:
            target_skills = [
                info for info in self._registry.list_skills()
                if info.name in skills
            ]
        
        if mode == "progressive":
            lines.append("\n> **Note**: Skill details are shown in summary mode.\n\n")
        
        for info in target_skills:
            skill_lines = self._format_skill_context(
                info,
                mode=mode,
                include_references=include_references,
                include_assets=include_assets,
                max_tokens=max_tokens_per_skill
            )
            lines.extend(skill_lines)
            lines.append("\n---\n")
        
        return "\n".join(lines)
    
    def _format_skill_context(
        self,
        info: SkillInfo,
        mode: str = "full",
        include_references: bool = False,
        include_assets: bool = False,
        max_tokens: Optional[int] = None
    ) -> List[str]:
        """Format a single skill's context based on the disclosure mode."""
        from ..parsing import get_skill_summary
        
        lines = [f"## {info.name}\n"]
        if info.description:
            lines.append(f"**Description:** {info.description}\n")
        
        if mode == "summary":
            full_content = info.get_full_content()
            if full_content:
                summary = get_skill_summary(full_content, max_length=150)
                if summary:
                    lines.append(f"\n**Summary:** {summary}\n")
        
        elif mode == "standard":
            if info.metadata.input_schema:
                lines.append("\n**Input Schema:**\n")
                schema_str = json.dumps(info.metadata.input_schema, indent=2, ensure_ascii=False)
                lines.append(f"```json\n{schema_str}\n```\n")
            full_content = info.get_full_content()
            if full_content:
                summary = get_skill_summary(full_content, max_length=200)
                if summary:
                    lines.append(f"\n**Usage:** {summary}\n")
        
        elif mode == "progressive":
            if info.metadata.input_schema:
                lines.append("\n**Input Schema:**\n")
                schema_str = json.dumps(info.metadata.input_schema, indent=2, ensure_ascii=False)
                lines.append(f"```json\n{schema_str}\n```\n")
            full_content = info.get_full_content()
            if full_content:
                summary = get_skill_summary(full_content, max_length=150)
                if summary:
                    lines.append(f"\n**Summary:** {summary}\n")
            has_more = bool(
                info.get_references() or
                info.get_assets() or
                (full_content and len(full_content) > 300)
            )
            if has_more:
                lines.append(f"\n> 💡 *More details available.*\n")
        
        else:  # full mode
            full_content = info.get_full_content()
            if full_content:
                if max_tokens and len(full_content) > max_tokens * 4:
                    full_content = full_content[:max_tokens * 4] + "\n\n... (truncated)"
                lines.append("\n### Instructions\n")
                lines.append(full_content)
                lines.append("\n")
        
        # Include references for full/standard modes
        if include_references and mode in ["full", "standard"]:
            refs = info.get_references()
            if refs:
                lines.append("\n### Reference Documents\n")
                for filename, content in refs.items():
                    lines.append(f"\n#### {filename}\n")
                    if max_tokens and len(content) > max_tokens * 2:
                        content = content[:max_tokens * 2] + "\n... (truncated)"
                    lines.append(content)
                    lines.append("\n")
        
        # Include assets for full/standard modes
        if include_assets and mode in ["full", "standard"]:
            assets = info.get_assets()
            if assets:
                lines.append("\n### Assets\n")
                for filename, content in assets.items():
                    lines.append(f"\n#### {filename}\n")
                    if isinstance(content, dict):
                        content_str = json.dumps(content, indent=2, ensure_ascii=False)
                        if max_tokens and len(content_str) > max_tokens * 2:
                            content_str = content_str[:max_tokens * 2] + "\n... (truncated)"
                        lines.append(f"```json\n{content_str}\n```\n")
                    else:
                        if max_tokens and len(str(content)) > max_tokens * 2:
                            content = str(content)[:max_tokens * 2] + "\n... (truncated)"
                        lines.append(f"```\n{content}\n```\n")
        
        return lines
    
    def get_skill_details(self, skill_name: str) -> Optional[str]:
        """Get full details for a specific skill."""
        info = self._registry.get_skill(skill_name)
        if not info:
            return None
        lines = self._format_skill_context(
            info,
            mode="full",
            include_references=True,
            include_assets=True
        )
        return "\n".join(lines)
    
    def get_skills_summary(self) -> str:
        """Get a compact summary of all available skills."""
        return self.get_system_prompt_context(mode="summary")
    
    def estimate_context_tokens(
        self,
        mode: str = "full",
        include_references: bool = False,
        include_assets: bool = False
    ) -> int:
        """Estimate the number of tokens the system prompt context will use."""
        context = self.get_system_prompt_context(
            mode=mode,
            include_references=include_references,
            include_assets=include_assets
        )
        return len(context) // 4
    
    def get_skill_context(self, skill_name: str) -> Optional[Dict[str, Any]]:
        """Get complete context for a specific skill."""
        info = self._registry.get_skill(skill_name)
        if not info:
            return None
        return info.get_context(include_references=True, include_assets=True)
    
    def get_all_skill_contexts(self) -> Dict[str, Dict[str, Any]]:
        """Get complete context for all skills."""
        return {
            info.name: info.get_context(include_references=True, include_assets=True)
            for info in self._registry.list_skills()
        }
    
    # ==================== Skills Status & Logging ====================
    
    def get_skills_status(self) -> Dict[str, Any]:
        """
        Get structured status information about all loaded skills.
        
        Returns a dict with:
        - all_skills: list of all skill names
        - executable_tools: list of executable skill/tool names
        - multi_script_tools: list of multi-script tool names
        - prompt_only_guides: list of prompt-only skill names
        - details: dict mapping skill name to its details
        """
        executable = self._registry.list_executable_skills()
        prompt_only = self._registry.list_prompt_only_skills()
        multi_script_tools = self._registry.list_multi_script_tools()
        
        # Build list of executable tool names
        executable_tool_names = []
        for info in executable:
            if info.metadata.entry_point:
                executable_tool_names.append(info.name)
        executable_tool_names.extend(multi_script_tools)
        
        details = {}
        for info in self._registry.list_skills():
            refs = info.get_references()
            assets = info.get_assets()
            scripts = info.get_all_scripts()
            
            is_multi_script = info.name in [
                t["skill_name"] for t in self._registry.multi_script_tools.values()
            ]
            
            details[info.name] = {
                "description": info.description,
                "is_executable": bool(info.metadata.entry_point) or is_multi_script,
                "is_multi_script": is_multi_script,
                "has_references": bool(refs),
                "has_assets": bool(assets),
                "references": list(refs.keys()) if refs else [],
                "assets": list(assets.keys()) if assets else [],
                "scripts": [s["name"] for s in scripts] if scripts else [],
            }
        
        return {
            "all_skills": self._registry.skill_names(),
            "executable_tools": executable_tool_names,
            "multi_script_tools": multi_script_tools,
            "prompt_only_guides": [s.name for s in prompt_only],
            "details": details,
        }
    
    def print_skills_status(self, verbose: bool = False) -> None:
        """Print a formatted status of all loaded skills."""
        status = self.get_skills_status()
        
        print(f"📦 已加载 Skills: {status['all_skills']}")
        
        if status["executable_tools"]:
            print(f"   🔧 可调用工具 (Tools): {status['executable_tools']}")
        
        if status["multi_script_tools"]:
            print(f"   🔨 多脚本工具 (Multi-script): {status['multi_script_tools']}")
        
        if status["prompt_only_guides"]:
            print(f"   📝 参考指南 (Prompt-only): {status['prompt_only_guides']}")
        
        if verbose:
            for name, detail in status["details"].items():
                extras = []
                if detail["has_references"]:
                    extras.append(f"refs={detail['references']}")
                if detail["has_assets"]:
                    extras.append(f"assets={detail['assets']}")
                if extras:
                    print(f"   📄 {name}: {', '.join(extras)}")
                if detail["description"]:
                    skill_type = "Tool" if detail["is_executable"] else "Guide"
                    print(f"      └─ [{skill_type}] {detail['description']}")
    
    def get_prompt_only_status(self) -> List[Dict[str, str]]:
        """Get status info for prompt-only skills."""
        return [
            {"name": s.name, "description": s.description or ""}
            for s in self._registry.list_prompt_only_skills()
        ]
    
    def print_prompt_only_status(self) -> None:
        """Print status of prompt-only skills."""
        prompt_only = self.get_prompt_only_status()
        if prompt_only:
            names = [s["name"] for s in prompt_only]
            print(f"📝 已注入参考指南 (Prompt-only Skills): {names}")
            for s in prompt_only:
                if s["description"]:
                    print(f"   └─ {s['name']}: {s['description']}")
