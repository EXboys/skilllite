"""
Planning rules for task planner - extensible rule-based tool selection.

Rules define when to use specific skills/tools based on user intent.
Load from config/planning_rules.json or provide custom rules at runtime.

Rule schema:
  - id: unique identifier
  - priority: higher = applied first (default 50)
  - keywords: optional list of trigger keywords (user message)
  - context_keywords: optional, for "continue" rules - context must mention these
  - tool_hint: suggested tool/skill name
  - instruction: prompt text for LLM (required)
"""

import json
from pathlib import Path
from typing import Any, Dict, List, Optional

_DEFAULT_PATH = Path(__file__).parent / "planning_rules.json"
_CACHE: Optional[List[Dict[str, Any]]] = None


def load_rules(path: Optional[Path] = None) -> List[Dict[str, Any]]:
    """Load planning rules from JSON file.

    Args:
        path: Path to JSON file. If None, use built-in config/planning_rules.json.

    Returns:
        List of rule dicts, sorted by priority descending.
    """
    p = path or _DEFAULT_PATH
    if not p.exists():
        return _builtin_rules()

    with open(p, encoding="utf-8") as f:
        data = json.load(f)

    rules = data.get("rules", [])
    if not rules:
        return _builtin_rules()

    # Sort by priority descending (higher first)
    def _priority(r: Dict) -> int:
        return r.get("priority", 50)

    return sorted(rules, key=_priority, reverse=True)


def _builtin_rules() -> List[Dict[str, Any]]:
    """Fallback when config file missing - minimal inline rules."""
    return [
        {"id": "explicit_skill", "priority": 100, "instruction": "**If user says \"使用 XX skill\" / \"用 XX 技能\" / \"use XX skills\"**, you MUST add that skill to the task list. Do NOT return empty list."},
        {"id": "weather", "priority": 90, "tool_hint": "weather", "instruction": "**天气/气象**: When user asks about weather, use **weather** skill. Return task with tool_hint: \"weather\"."},
        {"id": "realtime_http", "priority": 90, "tool_hint": "http-request", "instruction": "**实时/最新**: When user asks for real-time/latest data, use **http-request** skill. Return task with tool_hint: \"http-request\"."},
    ]


def get_rules(path: Optional[Path] = None, use_cache: bool = True) -> List[Dict[str, Any]]:
    """Get planning rules, with optional caching.

    Args:
        path: Optional override path. If None, use default.
        use_cache: If True and path is default, cache result.

    Returns:
        List of rule dicts.
    """
    global _CACHE
    if use_cache and path is None and _CACHE is not None:
        return _CACHE

    rules = load_rules(path)
    if use_cache and path is None:
        _CACHE = rules
    return rules


def build_rules_section(rules: Optional[List[Dict[str, Any]]] = None) -> str:
    """Build the '## CRITICAL' rules section for planning prompt.

    Args:
        rules: Rule list. If None, use get_rules().

    Returns:
        Formatted string to inject into planning prompt.
    """
    if rules is None:
        rules = get_rules()

    if not rules:
        return ""

    lines = [
        "## CRITICAL: When user explicitly requests a Skill, ALWAYS use it",
        "",
    ]
    for r in rules:
        inst = r.get("instruction", "").strip()
        if inst:
            lines.append(inst)
            lines.append("")

    return "\n".join(lines).rstrip()


def merge_rules(
    base: Optional[List[Dict[str, Any]]] = None,
    extra: Optional[List[Dict[str, Any]]] = None,
) -> List[Dict[str, Any]]:
    """Merge base rules with extra rules. Extra rules with same id override base.

    Args:
        base: Base rules (default from get_rules).
        extra: Additional/override rules.

    Returns:
        Merged list, sorted by priority.
    """
    base = base or get_rules()
    if not extra:
        return base

    by_id = {r["id"]: r for r in base}
    for r in extra:
        by_id[r["id"]] = r

    merged = list(by_id.values())
    return sorted(merged, key=lambda x: x.get("priority", 50), reverse=True)
