"""
Skill Registry - Skill registration and discovery.

This module handles:
- Scanning directories for skills
- Registering individual skills
- Skill lookup and listing
- Multi-script tool detection
"""

from pathlib import Path
from typing import Dict, List, Optional, Set

from .metadata import SkillMetadata, parse_skill_metadata, detect_all_scripts
from .skill_info import SkillInfo


class SkillRegistry:
    """
    Registry for managing skill registration and discovery.
    
    Handles skill scanning, registration, and lookup operations.
    Multi-script analysis is performed lazily when needed.
    """
    
    def __init__(self):
        self._skills: Dict[str, SkillInfo] = {}
        # Maps tool_name -> tool info for multi-script skills
        self._multi_script_tools: Dict[str, Dict[str, str]] = {}
        # Track which skills have been analyzed for multi-script tools
        self._analyzed_skills: Set[str] = set()
    
    def scan_directory(self, directory: Path) -> int:
        """
        Scan a directory for skills.
        
        Args:
            directory: Directory to scan
            
        Returns:
            Number of skills registered
            
        Raises:
            FileNotFoundError: If directory does not exist
        """
        if not directory.exists():
            raise FileNotFoundError(f"Skills directory does not exist: {directory}")
        
        # Check if directory itself is a skill
        if (directory / "SKILL.md").exists():
            self.register_skill(directory)
            return 1
        
        # Scan subdirectories
        count = 0
        for path in directory.iterdir():
            if path.is_dir() and (path / "SKILL.md").exists():
                try:
                    self.register_skill(path)
                    count += 1
                except Exception as e:
                    print(f"Warning: Failed to register skill at {path}: {e}")
        return count
    
    def register_skill(self, skill_dir: Path) -> SkillInfo:
        """
        Register a single skill from a directory.
        
        Multi-script analysis is deferred until needed (lazy loading).
        
        Args:
            skill_dir: Path to skill directory
            
        Returns:
            Registered SkillInfo
        """
        metadata = parse_skill_metadata(skill_dir)
        info = SkillInfo(metadata, skill_dir)
        self._skills[metadata.name] = info
        return info
    
    def analyze_multi_script_skill(self, skill_name: str) -> None:
        """
        Analyze a skill for multiple scripts and register them as tools.
        
        Called lazily when tool definitions are requested.
        
        Args:
            skill_name: Name of skill to analyze
        """
        if skill_name in self._analyzed_skills:
            return
        
        info = self._skills.get(skill_name)
        if not info:
            return
        
        # Only analyze skills without a main entry point
        if not info.metadata.entry_point:
            scripts = detect_all_scripts(info.path)
            if scripts:
                for script in scripts:
                    # Create unique tool name: skill-name__script-name
                    # Use double underscore instead of colon for API compatibility
                    tool_name = f"{skill_name}__{script['name']}"
                    self._multi_script_tools[tool_name] = {
                        "skill_name": skill_name,
                        "script_path": script["path"],
                        "script_name": script["name"],
                        "language": script["language"],
                        "filename": script["filename"],
                    }
        
        self._analyzed_skills.add(skill_name)
    
    def analyze_all_multi_script_skills(self) -> None:
        """Analyze all registered skills for multi-script tools."""
        for skill_name in self._skills:
            self.analyze_multi_script_skill(skill_name)
    
    def get_skill(self, name: str) -> Optional[SkillInfo]:
        """Get a skill by name."""
        return self._skills.get(name)
    
    def list_skills(self) -> List[SkillInfo]:
        """Get all registered skills."""
        return list(self._skills.values())
    
    def skill_names(self) -> List[str]:
        """Get names of all registered skills."""
        return list(self._skills.keys())
    
    def has_skill(self, name: str) -> bool:
        """Check if a skill exists."""
        return name in self._skills
    
    def is_executable(self, name: str) -> bool:
        """
        Check if a skill or tool is executable.
        
        Includes:
        - Skills with a single entry_point
        - Multi-script tools (skill-name:script-name format)
        - Bash-tool skills (allowed-tools: Bash(...))
        """
        if name in self._multi_script_tools:
            return True
        info = self._skills.get(name)
        if info is None:
            return False
        return bool(info.metadata.entry_point) or info.is_bash_tool_skill
    
    def list_executable_skills(self) -> List[SkillInfo]:
        """Get all executable skills (with entry_point, multi-script tools, or bash-tool skills)."""
        executable = []
        for info in self._skills.values():
            if info.metadata.entry_point:
                executable.append(info)
            elif info.is_bash_tool_skill:
                executable.append(info)
            elif info.name in [t["skill_name"] for t in self._multi_script_tools.values()]:
                executable.append(info)
        return executable
    
    def list_bash_tool_skills(self) -> List[SkillInfo]:
        """Get all bash-tool skills (with allowed-tools: Bash(...) and no entry_point)."""
        return [info for info in self._skills.values() if info.is_bash_tool_skill]
    
    def list_prompt_only_skills(self) -> List[SkillInfo]:
        """Get all prompt-only skills (without entry_point, no multi-script tools, and not bash-tool)."""
        prompt_only = []
        multi_script_skill_names = set(t["skill_name"] for t in self._multi_script_tools.values())
        for info in self._skills.values():
            if (not info.metadata.entry_point
                    and not info.is_bash_tool_skill
                    and info.name not in multi_script_skill_names):
                prompt_only.append(info)
        return prompt_only
    
    def list_multi_script_tools(self) -> List[str]:
        """Get all multi-script tool names."""
        return list(self._multi_script_tools.keys())
    
    def get_multi_script_tool_info(self, tool_name: str) -> Optional[Dict[str, str]]:
        """Get info for a multi-script tool."""
        return self._multi_script_tools.get(tool_name)
    
    @property
    def skills(self) -> Dict[str, SkillInfo]:
        """Direct access to skills dict (for compatibility)."""
        return self._skills
    
    @property
    def multi_script_tools(self) -> Dict[str, Dict[str, str]]:
        """Direct access to multi-script tools dict (for compatibility)."""
        return self._multi_script_tools
