"""
SkillInfo - Information container for a registered skill.

This is a CORE module - do not modify without explicit permission.

This module provides the SkillInfo class which encapsulates all information
about a skill including its metadata, content, references, and assets.
"""

import json
from pathlib import Path
from typing import Any, Dict, List, Optional

from .metadata import BashToolPattern, SkillMetadata, detect_language, detect_all_scripts, parse_allowed_tools

class SkillInfo:
    """Information about a registered skill."""
    
    def __init__(self, metadata: SkillMetadata, path: Path):
        self.metadata = metadata
        self.path = path
        self._full_content_cache: Optional[str] = None
    
    @property
    def name(self) -> str:
        return self.metadata.name
    
    @property
    def description(self) -> Optional[str]:
        return self.metadata.description
    
    @property
    def language(self) -> str:
        return detect_language(self.path, self.metadata)

    @property
    def is_bash_tool_skill(self) -> bool:
        """True if this skill uses `allowed-tools: Bash(...)` and has no script entry point."""
        return bool(self.metadata.allowed_tools) and not self.metadata.entry_point

    def get_bash_patterns(self) -> List[BashToolPattern]:
        """Parse the `allowed-tools` field into structured BashToolPattern items."""
        if self.metadata.allowed_tools:
            return parse_allowed_tools(self.metadata.allowed_tools)
        return []
    
    def get_full_content(self) -> str:
        """
        Get the full content of SKILL.md file.
        
        Returns:
            Complete content of SKILL.md including instructions and examples
        """
        if self._full_content_cache is not None:
            return self._full_content_cache
        
        skill_md_path = self.path / "SKILL.md"
        if skill_md_path.exists():
            self._full_content_cache = skill_md_path.read_text(encoding="utf-8")
            return self._full_content_cache
        return ""
    
    def get_references(self) -> Dict[str, str]:
        """
        Get all reference documents from references/ directory.
        
        Returns:
            Dictionary mapping filename to file content
        """
        references = {}
        references_dir = self.path / "references"
        
        if not references_dir.exists():
            return references
        
        for file_path in references_dir.iterdir():
            if file_path.is_file():
                try:
                    content = file_path.read_text(encoding="utf-8")
                    references[file_path.name] = content
                except Exception:
                    references[file_path.name] = f"[Error reading file: {file_path.name}]"
        
        return references
    
    def get_assets(self) -> Dict[str, Any]:
        """
        Get all asset files from assets/ directory.
        
        For JSON files, returns parsed content.
        For other files, returns file path for reference.
        
        Returns:
            Dictionary mapping filename to content or path
        """
        assets = {}
        assets_dir = self.path / "assets"
        
        if not assets_dir.exists():
            return assets
        
        for file_path in assets_dir.iterdir():
            if file_path.is_file():
                try:
                    if file_path.suffix.lower() == ".json":
                        content = file_path.read_text(encoding="utf-8")
                        assets[file_path.name] = json.loads(content)
                    elif file_path.suffix.lower() in [".txt", ".md", ".yaml", ".yml", ".toml", ".ini", ".cfg"]:
                        assets[file_path.name] = file_path.read_text(encoding="utf-8")
                    else:
                        assets[file_path.name] = {"_path": str(file_path), "_type": "binary"}
                except Exception as e:
                    assets[file_path.name] = {"_error": str(e)}
        
        return assets
    
    def get_assets_dir(self) -> Optional[Path]:
        """
        Get the path to assets/ directory if it exists.
        
        Returns:
            Path to assets directory, or None if not exists
        """
        assets_dir = self.path / "assets"
        return assets_dir if assets_dir.exists() else None
    
    def get_references_dir(self) -> Optional[Path]:
        """
        Get the path to references/ directory if it exists.
        
        Returns:
            Path to references directory, or None if not exists
        """
        references_dir = self.path / "references"
        return references_dir if references_dir.exists() else None
    
    def get_all_scripts(self) -> List[Dict[str, str]]:
        """
        Get all executable scripts in this skill.
        
        This is useful for skills with multiple entry points (like skill-creator
        which has init_skill.py, package_skill.py, etc.)
        
        Returns:
            List of dicts with 'name', 'path', 'language', and 'filename' for each script
        """
        return detect_all_scripts(self.path)
    
    def has_multiple_scripts(self) -> bool:
        """
        Check if this skill has multiple executable scripts.
        
        Returns:
            True if there are multiple scripts that could be entry points
        """
        scripts = self.get_all_scripts()
        return len(scripts) > 1
    
    def get_context(self, include_references: bool = True, include_assets: bool = True) -> Dict[str, Any]:
        """
        Get complete skill context for LLM consumption.
        
        Args:
            include_references: Whether to include reference documents
            include_assets: Whether to include asset files
            
        Returns:
            Dictionary containing full skill context
        """
        context = {
            "name": self.name,
            "description": self.description,
            "full_instructions": self.get_full_content(),
            "skill_dir": str(self.path),
        }
        
        if include_references:
            refs = self.get_references()
            if refs:
                context["references"] = refs
        
        if include_assets:
            assets = self.get_assets()
            if assets:
                context["assets"] = assets
        
        # Include available scripts info for multi-script skills
        scripts = self.get_all_scripts()
        if scripts:
            context["available_scripts"] = scripts
        
        return context
