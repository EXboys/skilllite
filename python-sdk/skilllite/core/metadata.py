"""
Skill metadata parsing from SKILL.md files.

Phase 4.12: Minimal parse for fallback; primary path uses skillbox list --json.
Keeps: get_skill_summary, from_list_json, parse_skill_metadata (slim).
"""

import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

import yaml

LOCK_FILE_NAME = ".skilllite.lock"


@dataclass
class NetworkPolicy:
    """Network access policy for a skill (derived from compatibility field)."""
    enabled: bool = False
    outbound: List[str] = field(default_factory=list)


@dataclass
class BashToolPattern:
    """Parsed pattern from `allowed-tools: Bash(agent-browser:*)`."""
    command_prefix: str
    raw_pattern: str


def parse_allowed_tools(raw: str) -> List[BashToolPattern]:
    """Parse the `allowed-tools` field into bash tool patterns."""
    patterns: List[BashToolPattern] = []
    for match in re.finditer(r"Bash\(([^)]+)\)", raw):
        inner = match.group(1).strip()
        command_prefix = inner[:inner.index(":")].strip() if ":" in inner else inner
        if command_prefix:
            patterns.append(BashToolPattern(command_prefix=command_prefix, raw_pattern=inner))
    return patterns


@dataclass
class SkillMetadata:
    """Skill metadata from SKILL.md or skillbox list --json."""
    name: str
    entry_point: str
    language: Optional[str] = None
    description: Optional[str] = None
    version: Optional[str] = None
    compatibility: Optional[str] = None
    network: NetworkPolicy = field(default_factory=NetworkPolicy)
    input_schema: Optional[Dict[str, Any]] = None
    output_schema: Optional[Dict[str, Any]] = None
    requires_elevated_permissions: bool = False
    resolved_packages: Optional[List[str]] = None
    allowed_tools: Optional[str] = None

    @classmethod
    def from_list_json(cls, data: Dict[str, Any]) -> "SkillMetadata":
        """Create SkillMetadata from skillbox list --json output."""
        entry_point = data.get("entry_point") or ""
        if not isinstance(entry_point, str):
            entry_point = ""
        compatibility = data.get("compatibility")
        network = parse_compatibility_for_network(compatibility)
        if not compatibility and data.get("network_enabled"):
            network = NetworkPolicy(enabled=True, outbound=["*:80", "*:443"])
        language = data.get("language") or parse_compatibility_for_language(compatibility)
        req = data.get("requires_elevated_permissions", False)
        if isinstance(req, str):
            req = req.lower() in ("true", "yes", "1")
        else:
            req = bool(req)
        return cls(
            name=data.get("name", ""),
            entry_point=entry_point,
            language=language,
            description=data.get("description"),
            version=None,
            compatibility=compatibility,
            network=network,
            input_schema=None,
            output_schema=None,
            requires_elevated_permissions=req,
            resolved_packages=data.get("resolved_packages"),
            allowed_tools=data.get("allowed_tools"),
        )


def parse_compatibility_for_network(compatibility: Optional[str]) -> NetworkPolicy:
    """Parse compatibility string for network policy."""
    if not compatibility:
        return NetworkPolicy()
    compat_lower = compatibility.lower()
    needs = any(k in compat_lower for k in ["network", "internet", "http", "api", "web"])
    return NetworkPolicy(enabled=needs, outbound=["*:80", "*:443"] if needs else [])


def parse_compatibility_for_language(compatibility: Optional[str]) -> Optional[str]:
    """Parse compatibility string for language."""
    if not compatibility:
        return None
    c = compatibility.lower()
    if "python" in c:
        return "python"
    if "node" in c or "javascript" in c or "typescript" in c:
        return "node"
    if "bash" in c or "shell" in c:
        return "bash"
    return None


def _detect_entry_point(skill_dir: Path) -> Optional[str]:
    """Minimal entry point detection: main.*, index.*, or single script."""
    scripts = skill_dir / "scripts"
    if not scripts.exists():
        return None
    exts = [".py", ".js", ".ts", ".sh"]
    for name in ["main", "index", "run", "entry"]:
        for ext in exts:
            if (scripts / f"{name}{ext}").exists():
                return f"scripts/{name}{ext}"
    files = [f for ext in exts for f in scripts.glob(f"*{ext}")
             if not f.name.startswith("test_") and f.name != "__init__.py" and not f.name.startswith(".")]
    return f"scripts/{files[0].name}" if len(files) == 1 else None


def _parse_yaml_minimal(content: str, skill_dir: Path) -> Dict[str, Any]:
    """Extract YAML front matter from SKILL.md content."""
    m = re.match(r"^---\n(.*?)\n---", content, re.DOTALL)
    if not m:
        return {}
    try:
        data = yaml.safe_load(m.group(1))
        return data if isinstance(data, dict) else {}
    except yaml.YAMLError:
        return {}


def parse_skill_metadata(skill_dir: Path) -> SkillMetadata:
    """Parse SKILL.md - minimal fallback when skillbox list --json unavailable."""
    path = skill_dir / "SKILL.md"
    if not path.exists():
        raise FileNotFoundError(f"SKILL.md not found in directory: {skill_dir}")
    content = path.read_text(encoding="utf-8")
    data = _parse_yaml_minimal(content, skill_dir)
    compatibility = data.get("compatibility")
    network = parse_compatibility_for_network(compatibility)
    entry_point = _detect_entry_point(skill_dir) or ""
    language = parse_compatibility_for_language(compatibility)
    if not language and entry_point:
        ext = Path(entry_point).suffix
        language = {".py": "python", ".js": "node", ".ts": "node", ".sh": "bash"}.get(ext)
    req = data.get("requires_elevated_permissions", False)
    if isinstance(req, str):
        req = req.lower() in ("true", "yes", "1")
    allowed = data.get("allowed-tools") or data.get("allowed_tools")
    return SkillMetadata(
        name=data.get("name", ""),
        entry_point=entry_point,
        language=language,
        description=data.get("description"),
        version=data.get("version") or (data.get("metadata") or {}).get("version"),
        compatibility=compatibility,
        network=network,
        input_schema=None,
        output_schema=None,
        requires_elevated_permissions=bool(req),
        resolved_packages=_read_resolved_packages(skill_dir, compatibility),
        allowed_tools=allowed,
    )


def _read_resolved_packages(skill_dir: Path, compatibility: Optional[str]) -> Optional[List[str]]:
    """Read resolved_packages from .skilllite.lock if fresh."""
    lock = skill_dir / LOCK_FILE_NAME
    if not lock.exists():
        return None
    try:
        data = json.loads(lock.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None
    import hashlib
    if data.get("compatibility_hash") != hashlib.sha256((compatibility or "").encode()).hexdigest():
        return None
    pkg = data.get("resolved_packages")
    return pkg if isinstance(pkg, list) and all(isinstance(x, str) for x in pkg) else None


def detect_all_scripts(skill_dir: Path) -> List[Dict[str, str]]:
    """Detect executable scripts in skill directory (for multi-script fallback)."""
    scripts = skill_dir / "scripts"
    if not scripts.exists():
        return []
    ext_lang = {".py": "python", ".js": "node", ".ts": "node", ".sh": "bash"}
    out = []
    for ext, lang in ext_lang.items():
        for f in scripts.glob(f"*{ext}"):
            if f.name.startswith("test_") or f.name.endswith("_test.py") or f.name == "__init__.py" or f.name.startswith("."):
                continue
            out.append({"name": f.stem.replace("_", "-"), "path": f"scripts/{f.name}", "language": lang, "filename": f.name})
    return out


def detect_language(skill_dir: Path, metadata: Optional[SkillMetadata] = None) -> str:
    """Detect skill language from metadata or scripts."""
    if metadata and metadata.language:
        return metadata.language
    if metadata and metadata.entry_point:
        ext = Path(metadata.entry_point).suffix
        m = {".py": "python", ".js": "node", ".ts": "node", ".sh": "bash"}
        if ext in m:
            return m[ext]
    scripts = skill_dir / "scripts"
    if scripts.exists():
        for ext, lang in [(".py", "python"), (".js", "node"), (".ts", "node"), (".sh", "bash")]:
            if any(scripts.glob(f"*{ext}")):
                return lang
    return "unknown"


def get_skill_summary(content: str, max_length: int = 200) -> str:
    """Extract concise summary from SKILL.md (removes YAML, code blocks, headers)."""
    content = re.sub(r"^---\s*\n.*?\n---\s*\n", "", content, flags=re.DOTALL)
    content = re.sub(r"```[\s\S]*?```", "", content)
    content = re.sub(r"^#+\s*", "", content, flags=re.MULTILINE)
    lines = [l.strip() for l in content.split("\n") if l.strip()]
    out, n = [], 0
    for line in lines:
        if n + len(line) > max_length:
            break
        out.append(line)
        n += len(line) + 1
    return " ".join(out)[:max_length]


# --- Skill name validation (from validation.py merge) ---

def validate_skill_name(name: str, skill_dir: Optional[Path] = None) -> tuple:
    """Validate skill name per Agent Skills spec. Returns (is_valid, errors)."""
    errors = []
    if not name:
        errors.append("Skill name cannot be empty")
    elif len(name) > 64:
        errors.append(f"Skill name exceeds 64 characters (got {len(name)})")
    if name and not re.match(r"^[a-z0-9-]+$", name):
        invalid = set(re.findall(r"[^a-z0-9-]", name))
        errors.append(f"Skill name contains invalid characters: {invalid}")
    if name and (name.startswith("-") or name.endswith("-") or "--" in name):
        errors.append("Skill name must not start/end with hyphen or contain consecutive hyphens")
    if name and skill_dir and skill_dir.name != name:
        errors.append(f"Skill name must match directory name (got {name}, dir is {skill_dir.name})")
    return (len(errors) == 0, errors)


def validate_skill_name_strict(name: str, skill_dir: Optional[Path] = None) -> None:
    """Raise if invalid."""
    valid, errors = validate_skill_name(name, skill_dir)
    if not valid:
        raise ValueError(f"Invalid skill name: {'; '.join(errors)}")


def validate_skill_name_warn(name: str, skill_dir: Optional[Path] = None) -> bool:
    """Warn if invalid. Returns True if valid."""
    import warnings
    valid, errors = validate_skill_name(name, skill_dir)
    if not valid:
        warnings.warn(f"Skill name validation: {errors[0]}", UserWarning, stacklevel=2)
    return valid
