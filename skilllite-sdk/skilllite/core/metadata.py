"""
Skill metadata parsing from SKILL.md files.

This is a CORE module - do not modify without explicit permission.

Follows the official Claude Agent Skills specification:
https://docs.anthropic.com/en/docs/agents-and-tools/agent-skills/specification

Network access and language are derived from the 'compatibility' field.
Entry point is auto-detected from scripts/ directory.
"""

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

import yaml

@dataclass
class NetworkPolicy:
    """Network access policy for a skill (derived from compatibility field)."""
    enabled: bool = False
    outbound: List[str] = field(default_factory=list)

@dataclass
class SkillMetadata:
    """Skill metadata parsed from SKILL.md YAML front matter."""
    name: str
    entry_point: str
    language: Optional[str] = None
    description: Optional[str] = None
    version: Optional[str] = None
    compatibility: Optional[str] = None
    network: NetworkPolicy = field(default_factory=NetworkPolicy)
    input_schema: Optional[Dict[str, Any]] = None
    output_schema: Optional[Dict[str, Any]] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any], skill_dir: Optional[Path] = None) -> "SkillMetadata":
        """Create SkillMetadata from parsed YAML front matter."""
        version = data.get("version")
        if not version and "metadata" in data:
            version = data["metadata"].get("version")
        
        compatibility = data.get("compatibility")
        
        # Parse network policy from compatibility field
        network = parse_compatibility_for_network(compatibility)
        
        # Auto-detect entry point
        entry_point = ""
        if skill_dir:
            detected = detect_entry_point(skill_dir)
            if detected:
                entry_point = detected
        
        # Detect language from compatibility or entry point
        language = parse_compatibility_for_language(compatibility)
        if not language and entry_point:
            language = detect_language_from_entry_point(entry_point)
        
        return cls(
            name=data.get("name", ""),
            entry_point=entry_point,
            language=language,
            description=data.get("description"),
            version=version,
            compatibility=compatibility,
            network=network,
            input_schema=data.get("input_schema"),
            output_schema=data.get("output_schema")
        )


def parse_compatibility_for_network(compatibility: Optional[str]) -> NetworkPolicy:
    """
    Parse compatibility string to extract network policy.
    
    Examples:
        - "Requires network access" -> enabled=True
        - "Requires Python 3.x, internet" -> enabled=True
        - "Requires git, docker" -> enabled=False
    """
    if not compatibility:
        return NetworkPolicy()
    
    compat_lower = compatibility.lower()
    
    # Check for network/internet keywords
    needs_network = any(keyword in compat_lower for keyword in [
        "network", "internet", "http", "api", "web"
    ])
    
    if needs_network:
        return NetworkPolicy(
            enabled=True,
            outbound=["*:80", "*:443"]  # Allow all HTTP/HTTPS by default
        )
    
    return NetworkPolicy()


def parse_compatibility_for_language(compatibility: Optional[str]) -> Optional[str]:
    """
    Parse compatibility string to detect language.
    
    Examples:
        - "Requires Python 3.x" -> "python"
        - "Requires Node.js" -> "node"
        - "Requires bash" -> "bash"
    """
    if not compatibility:
        return None
    
    compat_lower = compatibility.lower()
    
    if "python" in compat_lower:
        return "python"
    elif "node" in compat_lower or "javascript" in compat_lower or "typescript" in compat_lower:
        return "node"
    elif "bash" in compat_lower or "shell" in compat_lower:
        return "bash"
    
    return None


def detect_language_from_entry_point(entry_point: str) -> Optional[str]:
    """Detect language from entry point file extension."""
    if entry_point.endswith(".py"):
        return "python"
    elif entry_point.endswith(".js") or entry_point.endswith(".ts"):
        return "node"
    elif entry_point.endswith(".sh"):
        return "bash"
    return None

def detect_entry_point(skill_dir: Path) -> Optional[str]:
    """
    Auto-detect entry point from skill directory.
    
    Detection strategy (in order of priority):
    1. Look for main.* files (main.py, main.js, main.ts, main.sh)
    2. Look for index.* files (common in Node.js projects)
    3. Look for run.* or entry.* files
    4. If only one script file exists, use it as entry point
    5. If multiple scripts exist, return None (requires explicit config or LLM inference)
    
    Returns:
        Relative path to entry point (e.g., "scripts/main.py"), or None if not detected
    """
    scripts_dir = skill_dir / "scripts"
    if not scripts_dir.exists():
        return None
    
    supported_extensions = [".py", ".js", ".ts", ".sh"]
    
    # Priority 1: Look for main.* files
    for ext in supported_extensions:
        main_file = scripts_dir / f"main{ext}"
        if main_file.exists():
            return f"scripts/main{ext}"
    
    # Priority 2: Look for index.* files (common in Node.js)
    for ext in supported_extensions:
        index_file = scripts_dir / f"index{ext}"
        if index_file.exists():
            return f"scripts/index{ext}"
    
    # Priority 3: Look for run.* or entry.* files
    for prefix in ["run", "entry", "app", "cli"]:
        for ext in supported_extensions:
            candidate = scripts_dir / f"{prefix}{ext}"
            if candidate.exists():
                return f"scripts/{prefix}{ext}"
    
    # Priority 4: If only one script file exists, use it
    script_files = []
    for ext in supported_extensions:
        script_files.extend(scripts_dir.glob(f"*{ext}"))
    
    # Filter out test files and __init__.py
    script_files = [
        f for f in script_files 
        if not f.name.startswith("test_") 
        and not f.name.endswith("_test.py")
        and f.name != "__init__.py"
        and not f.name.startswith(".")
    ]
    
    if len(script_files) == 1:
        return f"scripts/{script_files[0].name}"
    
    # Multiple scripts or no scripts found - return None
    # This will be handled by LLM inference or explicit configuration
    return None

def detect_all_scripts(skill_dir: Path) -> List[Dict[str, str]]:
    """
    Detect all executable scripts in a skill directory.
    
    This is useful for skills with multiple entry points (like skill-creator
    which has init_skill.py, package_skill.py, etc.)
    
    Returns:
        List of dicts with 'name', 'path', and 'language' for each script
    """
    scripts_dir = skill_dir / "scripts"
    if not scripts_dir.exists():
        return []
    
    extension_to_language = {
        ".py": "python",
        ".js": "node",
        ".ts": "node",
        ".sh": "bash",
    }
    
    scripts = []
    for ext, lang in extension_to_language.items():
        for script_file in scripts_dir.glob(f"*{ext}"):
            # Skip test files and __init__.py
            if (script_file.name.startswith("test_") 
                or script_file.name.endswith("_test.py")
                or script_file.name == "__init__.py"
                or script_file.name.startswith(".")):
                continue
            
            # Generate a tool name from the script filename
            tool_name = script_file.stem.replace("_", "-")
            
            scripts.append({
                "name": tool_name,
                "path": f"scripts/{script_file.name}",
                "language": lang,
                "filename": script_file.name,
            })
    
    return scripts

def detect_language(skill_dir: Path, metadata: Optional[SkillMetadata] = None) -> str:
    """
    Detect the programming language of a skill.
    
    Args:
        skill_dir: Path to the skill directory
        metadata: Optional metadata object (may contain language info)
        
    Returns:
        Language string (e.g., "python", "node", "bash")
    """
    # First check metadata
    if metadata and metadata.language:
        return metadata.language
    
    # Check entry point extension
    if metadata and metadata.entry_point:
        entry_ext = Path(metadata.entry_point).suffix
        ext_map = {".py": "python", ".js": "node", ".ts": "node", ".sh": "bash"}
        if entry_ext in ext_map:
            return ext_map[entry_ext]
    
    # Scan scripts directory
    scripts_dir = skill_dir / "scripts"
    if scripts_dir.exists():
        for ext, lang in [(".py", "python"), (".js", "node"), (".ts", "node"), (".sh", "bash")]:
            if any(scripts_dir.glob(f"*{ext}")):
                return lang
    
    return "unknown"

def extract_yaml_front_matter(content: str, skill_dir: Optional[Path] = None) -> SkillMetadata:
    """
    Extract YAML front matter from markdown content.
    
    Args:
        content: Full markdown content
        skill_dir: Optional skill directory path for auto-detection
        
    Returns:
        SkillMetadata object
    """
    # Check for YAML front matter (between --- markers)
    front_matter_match = re.match(r'^---\n(.*?)\n---', content, re.DOTALL)
    
    data = {}
    if front_matter_match:
        try:
            data = yaml.safe_load(front_matter_match.group(1))
            if not isinstance(data, dict):
                data = {}
        except yaml.YAMLError:
            data = {}
    
    return SkillMetadata.from_dict(data, skill_dir)

def parse_skill_metadata(skill_dir: Path) -> SkillMetadata:
    """Parse SKILL.md file and extract metadata from YAML front matter."""
    skill_md_path = skill_dir / "SKILL.md"
    if not skill_md_path.exists():
        raise FileNotFoundError(f"SKILL.md not found in directory: {skill_dir}")
    content = skill_md_path.read_text(encoding="utf-8")
    metadata = extract_yaml_front_matter(content, skill_dir)
    
    return metadata

def get_skill_summary(content: str, max_length: int = 200) -> str:
    """
    Extract a concise summary from SKILL.md content.
    
    Removes YAML front matter, code blocks, and headers to extract
    the main descriptive text.
    
    Args:
        content: Full SKILL.md content
        max_length: Maximum length of the summary
        
    Returns:
        Extracted summary string
    """
    # Remove YAML front matter
    content_clean = re.sub(r"^---\s*\n.*?\n---\s*\n", "", content, flags=re.DOTALL)
    # Remove code blocks
    content_clean = re.sub(r"```[\s\S]*?```", "", content_clean)
    # Remove headers
    content_clean = re.sub(r"^#+\s*", "", content_clean, flags=re.MULTILINE)
    
    lines = [line.strip() for line in content_clean.split("\n") if line.strip()]
    summary_lines = []
    current_length = 0
    
    for line in lines:
        if current_length + len(line) > max_length:
            break
        summary_lines.append(line)
        current_length += len(line) + 1
    
    return " ".join(summary_lines)[:max_length]
