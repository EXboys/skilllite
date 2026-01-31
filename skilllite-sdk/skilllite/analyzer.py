"""
Script Analyzer - Analyzes skill scripts and generates execution recommendations for LLM.

This module provides tools to scan skill directories, analyze scripts, and generate
structured information that can be used by LLMs to decide how to execute skills.
"""

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from .sandbox.skillbox import ensure_installed


@dataclass
class ScriptInfo:
    """Information about a single script file."""
    path: str
    language: str
    total_lines: int
    preview: str
    description: Optional[str]
    has_main_entry: bool
    uses_argparse: bool
    uses_stdio: bool
    file_size_bytes: int

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ScriptInfo":
        return cls(
            path=data.get("path", ""),
            language=data.get("language", ""),
            total_lines=data.get("total_lines", 0),
            preview=data.get("preview", ""),
            description=data.get("description"),
            has_main_entry=data.get("has_main_entry", False),
            uses_argparse=data.get("uses_argparse", False),
            uses_stdio=data.get("uses_stdio", False),
            file_size_bytes=data.get("file_size_bytes", 0),
        )


@dataclass
class SkillScanResult:
    """Result of scanning a skill directory."""
    skill_dir: str
    has_skill_md: bool
    skill_metadata: Optional[Dict[str, Any]]
    scripts: List[ScriptInfo]
    directories: Dict[str, bool]
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SkillScanResult":
        scripts = [ScriptInfo.from_dict(s) for s in data.get("scripts", [])]
        return cls(
            skill_dir=data.get("skill_dir", ""),
            has_skill_md=data.get("has_skill_md", False),
            skill_metadata=data.get("skill_metadata"),
            scripts=scripts,
            directories=data.get("directories", {}),
        )


@dataclass
class ExecutionRecommendation:
    """Recommendation for how to execute a script."""
    script_path: str
    language: str
    execution_method: str  # "stdin_json", "argparse", "direct"
    confidence: float  # 0.0 to 1.0
    reasoning: str
    suggested_command: str
    input_format: str  # "json_stdin", "cli_args", "none"
    output_format: str  # "json_stdout", "text_stdout", "file"


class ScriptAnalyzer:
    """
    Analyzes skill directories and scripts to provide execution recommendations.
    
    This class uses the skillbox binary to scan directories and then provides
    additional analysis and recommendations for LLM-based execution decisions.
    """
    
    def __init__(self, binary_path: Optional[str] = None, auto_install: bool = False):
        """
        Initialize the analyzer.
        
        Args:
            binary_path: Path to the skillbox binary. If None, auto-detect.
            auto_install: Automatically download and install binary if not found.
        """
        self.binary_path = binary_path or ensure_installed(auto_install=auto_install)
    
    def scan(self, skill_dir: Path, preview_lines: int = 10) -> SkillScanResult:
        """
        Scan a skill directory and return information about all scripts.
        
        Args:
            skill_dir: Path to the skill directory
            preview_lines: Number of lines to include in script preview
            
        Returns:
            SkillScanResult with information about the skill and its scripts
        """
        cmd = [
            self.binary_path,
            "scan",
            str(skill_dir),
            "--preview-lines",
            str(preview_lines),
        ]
        
        result = subprocess.run(cmd, capture_output=True, text=True)
        
        if result.returncode != 0:
            raise RuntimeError(f"Failed to scan skill directory: {result.stderr}")
        
        data = json.loads(result.stdout)
        return SkillScanResult.from_dict(data)
    
    def analyze_for_execution(
        self, 
        skill_dir: Path,
        task_description: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Analyze a skill directory and generate execution recommendations.
        
        This method is designed to produce output that can be directly used
        by an LLM to decide how to execute scripts in the skill.
        
        Args:
            skill_dir: Path to the skill directory
            task_description: Optional description of what the user wants to do
            
        Returns:
            Dictionary with analysis results suitable for LLM consumption
        """
        scan_result = self.scan(skill_dir, preview_lines=15)
        
        recommendations = []
        for script in scan_result.scripts:
            rec = self._analyze_script(script, scan_result.skill_metadata)
            recommendations.append(rec)
        
        # Sort by confidence
        recommendations.sort(key=lambda r: r.confidence, reverse=True)
        
        return {
            "skill_dir": str(skill_dir),
            "skill_name": scan_result.skill_metadata.get("name") if scan_result.skill_metadata else None,
            "skill_description": scan_result.skill_metadata.get("description") if scan_result.skill_metadata else None,
            "has_skill_md": scan_result.has_skill_md,
            "total_scripts": len(scan_result.scripts),
            "directories": scan_result.directories,
            "recommendations": [
                {
                    "script_path": r.script_path,
                    "language": r.language,
                    "execution_method": r.execution_method,
                    "confidence": r.confidence,
                    "reasoning": r.reasoning,
                    "suggested_command": r.suggested_command,
                    "input_format": r.input_format,
                    "output_format": r.output_format,
                }
                for r in recommendations
            ],
            "scripts_detail": [
                {
                    "path": s.path,
                    "language": s.language,
                    "description": s.description,
                    "total_lines": s.total_lines,
                    "has_main_entry": s.has_main_entry,
                    "uses_argparse": s.uses_argparse,
                    "uses_stdio": s.uses_stdio,
                }
                for s in scan_result.scripts
            ],
            "llm_prompt_hint": self._generate_llm_hint(scan_result, task_description),
        }
    
    def _analyze_script(
        self, 
        script: ScriptInfo, 
        skill_metadata: Optional[Dict[str, Any]]
    ) -> ExecutionRecommendation:
        """Analyze a single script and generate execution recommendation."""
        
        confidence = 0.5
        reasoning_parts = []
        
        # Determine execution method based on script characteristics
        if script.uses_stdio and not script.uses_argparse:
            execution_method = "stdin_json"
            input_format = "json_stdin"
            confidence += 0.3
            reasoning_parts.append("Script uses stdin/stdout for I/O")
        elif script.uses_argparse:
            execution_method = "argparse"
            input_format = "cli_args"
            confidence += 0.2
            reasoning_parts.append("Script uses argument parsing")
        else:
            execution_method = "direct"
            input_format = "none"
            reasoning_parts.append("Script appears to run directly without input")
        
        # Boost confidence for scripts with main entry
        if script.has_main_entry:
            confidence += 0.1
            reasoning_parts.append("Has main entry point")
        
        # Boost confidence for scripts in scripts/ directory
        if script.path.startswith("scripts/"):
            confidence += 0.1
            reasoning_parts.append("Located in scripts/ directory")
        
        # Check if this matches the skill's entry_point
        if skill_metadata and skill_metadata.get("entry_point") == script.path:
            confidence = 1.0
            reasoning_parts.insert(0, "Matches skill entry_point")
        
        # Determine output format
        if script.uses_stdio:
            output_format = "json_stdout" if "json" in script.preview.lower() else "text_stdout"
        else:
            output_format = "text_stdout"
        
        # Generate suggested command
        suggested_command = self._generate_command(script, execution_method)
        
        return ExecutionRecommendation(
            script_path=script.path,
            language=script.language,
            execution_method=execution_method,
            confidence=min(confidence, 1.0),
            reasoning="; ".join(reasoning_parts),
            suggested_command=suggested_command,
            input_format=input_format,
            output_format=output_format,
        )
    
    def _generate_command(self, script: ScriptInfo, execution_method: str) -> str:
        """Generate a suggested command for executing the script."""
        
        if execution_method == "stdin_json":
            if script.language == "python":
                return f'echo \'{{"input": "value"}}\' | python {script.path}'
            elif script.language == "node":
                return f'echo \'{{"input": "value"}}\' | node {script.path}'
            elif script.language == "shell":
                return f'echo \'{{"input": "value"}}\' | bash {script.path}'
        elif execution_method == "argparse":
            if script.language == "python":
                return f'python {script.path} --help'
            elif script.language == "node":
                return f'node {script.path} --help'
            elif script.language == "shell":
                return f'bash {script.path} --help'
        else:
            if script.language == "python":
                return f'python {script.path}'
            elif script.language == "node":
                return f'node {script.path}'
            elif script.language == "shell":
                return f'bash {script.path}'
        
        return f'# Unknown execution method for {script.path}'
    
    def _generate_llm_hint(
        self, 
        scan_result: SkillScanResult, 
        task_description: Optional[str]
    ) -> str:
        """Generate a hint for LLM to understand how to use this skill."""
        
        hints = []
        
        if scan_result.skill_metadata:
            meta = scan_result.skill_metadata
            if meta.get("description"):
                hints.append(f"Skill purpose: {meta['description']}")
            if meta.get("entry_point"):
                hints.append(f"Primary entry point: {meta['entry_point']}")
        
        if not scan_result.scripts:
            hints.append("No executable scripts found. This may be a prompt-only skill.")
        else:
            script_types = {}
            for s in scan_result.scripts:
                script_types[s.language] = script_types.get(s.language, 0) + 1
            
            type_str = ", ".join(f"{count} {lang}" for lang, count in script_types.items())
            hints.append(f"Available scripts: {type_str}")
            
            # Highlight scripts with descriptions
            described = [s for s in scan_result.scripts if s.description]
            if described:
                hints.append("Scripts with descriptions:")
                for s in described[:3]:  # Limit to 3
                    hints.append(f"  - {s.path}: {s.description[:100]}")
        
        if task_description:
            hints.append(f"User task: {task_description}")
        
        return "\n".join(hints)
    
    def get_execution_context(self, skill_dir: Path) -> Dict[str, Any]:
        """
        Get execution context for a skill, suitable for passing to skillbox exec.
        
        Returns a dictionary with all information needed to execute scripts
        in the skill directory.
        """
        scan_result = self.scan(skill_dir)
        
        return {
            "skill_dir": str(skill_dir.absolute()),
            "has_skill_md": scan_result.has_skill_md,
            "network_enabled": (
                scan_result.skill_metadata.get("network_enabled", False)
                if scan_result.skill_metadata else False
            ),
            "compatibility": (
                scan_result.skill_metadata.get("compatibility")
                if scan_result.skill_metadata else None
            ),
            "available_scripts": [
                {
                    "path": s.path,
                    "language": s.language,
                    "has_main": s.has_main_entry,
                    "uses_argparse": s.uses_argparse,
                    "uses_stdio": s.uses_stdio,
                }
                for s in scan_result.scripts
            ],
        }


def scan_skill(skill_dir: str, preview_lines: int = 10) -> Dict[str, Any]:
    """
    Convenience function to scan a skill directory.
    
    Args:
        skill_dir: Path to the skill directory
        preview_lines: Number of lines to include in script preview
        
    Returns:
        Dictionary with scan results
    """
    analyzer = ScriptAnalyzer(auto_install=True)
    result = analyzer.scan(Path(skill_dir), preview_lines)
    return {
        "skill_dir": result.skill_dir,
        "has_skill_md": result.has_skill_md,
        "skill_metadata": result.skill_metadata,
        "scripts": [
            {
                "path": s.path,
                "language": s.language,
                "description": s.description,
                "total_lines": s.total_lines,
                "has_main_entry": s.has_main_entry,
                "uses_argparse": s.uses_argparse,
                "uses_stdio": s.uses_stdio,
            }
            for s in result.scripts
        ],
        "directories": result.directories,
    }


def analyze_skill(skill_dir: str, task_description: Optional[str] = None) -> Dict[str, Any]:
    """
    Convenience function to analyze a skill for execution.
    
    Args:
        skill_dir: Path to the skill directory
        task_description: Optional description of what the user wants to do
        
    Returns:
        Dictionary with analysis results and recommendations
    """
    analyzer = ScriptAnalyzer(auto_install=True)
    return analyzer.analyze_for_execution(Path(skill_dir), task_description)
