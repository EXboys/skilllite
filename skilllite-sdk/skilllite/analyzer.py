"""
Script Analyzer - Analyzes skill scripts and generates execution recommendations for LLM.

This module delegates to skillbox scan for all analysis logic. Python provides
thin wrappers and dataclasses for API compatibility.
"""

import json
import subprocess
from dataclasses import dataclass
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

    Delegates all analysis to skillbox scan. Python provides thin wrappers.
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

        Delegates to skillbox scan. Returns structured output for LLM consumption.

        Args:
            skill_dir: Path to the skill directory
            task_description: Optional description of what the user wants to do

        Returns:
            Dictionary with analysis results suitable for LLM consumption
        """
        cmd = [
            self.binary_path,
            "scan",
            str(skill_dir),
            "--preview-lines",
            "15",
        ]

        result = subprocess.run(cmd, capture_output=True, text=True)

        if result.returncode != 0:
            raise RuntimeError(f"Failed to scan skill directory: {result.stderr}")

        data = json.loads(result.stdout)
        scripts = data.get("scripts", [])

        # Map skillbox output to recommendations (already includes reasoning, suggested_command, etc.)
        recommendations = []
        for s in scripts:
            recommendations.append({
                "script_path": s.get("path", ""),
                "language": s.get("language", ""),
                "execution_method": s.get("execution_recommendation", "direct"),
                "confidence": s.get("confidence", 0.0),
                "reasoning": s.get("reasoning", ""),
                "suggested_command": s.get("suggested_command", ""),
                "input_format": s.get("input_format", "none"),
                "output_format": s.get("output_format", "text_stdout"),
            })

        # Sort by confidence
        recommendations.sort(key=lambda r: r["confidence"], reverse=True)

        # Build llm_prompt_hint (skillbox provides base; append task_description if given)
        llm_hint = data.get("llm_prompt_hint", "")
        if task_description:
            llm_hint = f"{llm_hint}\nUser task: {task_description}" if llm_hint else f"User task: {task_description}"

        return {
            "skill_dir": str(skill_dir),
            "skill_name": (data.get("skill_metadata") or {}).get("name"),
            "skill_description": (data.get("skill_metadata") or {}).get("description"),
            "has_skill_md": data.get("has_skill_md", False),
            "total_scripts": len(scripts),
            "directories": data.get("directories", {}),
            "recommendations": recommendations,
            "scripts_detail": [
                {
                    "path": s.get("path", ""),
                    "language": s.get("language", ""),
                    "description": s.get("description"),
                    "total_lines": s.get("total_lines", 0),
                    "has_main_entry": s.get("has_main_entry", False),
                    "uses_argparse": s.get("uses_argparse", False),
                    "uses_stdio": s.get("uses_stdio", False),
                }
                for s in scripts
            ],
            "llm_prompt_hint": llm_hint,
        }

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
