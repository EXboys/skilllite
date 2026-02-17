"""
Configuration — env parsing, package whitelist, planning rules.
"""

import json
import os
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# --- Env config ---

def parse_bool_env(key: str, default: bool, legacy_key: Optional[str] = None) -> bool:
    value = os.environ.get(key) or (os.environ.get(legacy_key) if legacy_key else None)
    if value is None: return default
    v = value.lower().strip()
    if v in ("true", "1", "yes", "on"): return True
    if v in ("false", "0", "no", "off", ""): return False
    return default


def get_int_env(key: str, default: int, legacy_key: Optional[str] = None) -> int:
    value = os.environ.get(key) or (os.environ.get(legacy_key) if legacy_key else None)
    if value:
        try: return int(value)
        except ValueError: pass
    return default


def get_timeout_from_env() -> int:
    return get_int_env("SKILLBOX_TIMEOUT_SECS", 120, "EXECUTION_TIMEOUT")


def get_memory_from_env() -> int:
    return get_int_env("SKILLBOX_MAX_MEMORY_MB", 512, "MAX_MEMORY_MB")


def get_long_text_chunk_size() -> int:
    return get_int_env("SKILLLITE_CHUNK_SIZE", 6000)


def get_long_text_head_chunks() -> int:
    return get_int_env("SKILLLITE_HEAD_CHUNKS", 3)


def get_long_text_tail_chunks() -> int:
    return get_int_env("SKILLLITE_TAIL_CHUNKS", 3)


def get_long_text_max_output_chars() -> int:
    return get_int_env("SKILLLITE_MAX_OUTPUT_CHARS", 8000)


def get_long_text_summarize_threshold() -> int:
    return get_int_env("SKILLLITE_SUMMARIZE_THRESHOLD", 15000)


def get_tool_result_max_chars() -> int:
    return get_int_env("SKILLLITE_TOOL_RESULT_MAX_CHARS", 8000)


def get_planning_rules_path() -> Optional[str]:
    return os.environ.get("SKILLLITE_PLANNING_RULES_PATH")


# --- Package whitelist ---

_WHITELIST_PATH = Path(__file__).parent / "packages_whitelist.json"
_WHITELIST_CACHE: Optional[dict] = None


def _load_whitelist() -> dict:
    global _WHITELIST_CACHE
    if _WHITELIST_CACHE is None:
        with open(_WHITELIST_PATH, encoding="utf-8") as f:
            _WHITELIST_CACHE = json.load(f)
    return _WHITELIST_CACHE


def get_python_packages() -> List[str]:
    return list(_load_whitelist().get("python", []))


def get_python_aliases() -> List[str]:
    return list(_load_whitelist().get("python_aliases", []))


def get_node_packages() -> List[str]:
    return list(_load_whitelist().get("node", []))


def get_all_packages() -> Tuple[List[str], List[str]]:
    return get_python_packages(), get_node_packages()


# --- Planning rules ---

_RULES_PATH = Path(__file__).parent / "planning_rules.json"
_RULES_CACHE: Optional[List[Dict[str, Any]]] = None


def _builtin_rules() -> List[Dict[str, Any]]:
    return [
        {"id": "explicit_skill", "priority": 100, "instruction": "**If user says \"使用 XX skill\" / \"用 XX 技能\" / \"use XX skills\"**, you MUST add that skill to the task list. Do NOT return empty list."},
        {"id": "weather", "priority": 90, "tool_hint": "weather", "instruction": "**天气/气象**: When user asks about weather, use **weather** skill. Return task with tool_hint: \"weather\"."},
        {"id": "realtime_http", "priority": 90, "tool_hint": "http-request", "instruction": "**实时/最新**: When user asks for real-time/latest data, use **http-request** skill. Return task with tool_hint: \"http-request\"."},
    ]


def load_rules(path: Optional[Path] = None) -> List[Dict[str, Any]]:
    p = path or _RULES_PATH
    if not p.exists(): return _builtin_rules()
    with open(p, encoding="utf-8") as f:
        rules = json.load(f).get("rules", [])
    if not rules: return _builtin_rules()
    return sorted(rules, key=lambda r: r.get("priority", 50), reverse=True)


def get_rules(path: Optional[Path] = None, use_cache: bool = True) -> List[Dict[str, Any]]:
    global _RULES_CACHE
    if use_cache and path is None and _RULES_CACHE is not None: return _RULES_CACHE
    rules = load_rules(path)
    if use_cache and path is None: _RULES_CACHE = rules
    return rules


def build_rules_section(rules: Optional[List[Dict[str, Any]]] = None) -> str:
    if rules is None: rules = get_rules()
    if not rules: return ""
    lines = ["## CRITICAL: When user explicitly requests a Skill, ALWAYS use it", ""]
    for r in rules:
        inst = r.get("instruction", "").strip()
        if inst: lines.extend([inst, ""])
    return "\n".join(lines).rstrip()


def merge_rules(base: Optional[List[Dict[str, Any]]] = None, extra: Optional[List[Dict[str, Any]]] = None) -> List[Dict[str, Any]]:
    base = base or get_rules()
    if not extra: return base
    by_id = {r["id"]: r for r in base}
    for r in extra: by_id[r["id"]] = r
    return sorted(by_id.values(), key=lambda x: x.get("priority", 50), reverse=True)


__all__ = [
    "parse_bool_env", "get_int_env", "get_timeout_from_env", "get_memory_from_env",
    "get_python_packages", "get_python_aliases", "get_node_packages", "get_all_packages",
    "get_rules", "load_rules", "build_rules_section", "merge_rules",
]
