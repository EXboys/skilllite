"""
Unified environment variable parsing for SkillLite.

Single source of truth for reading SKILLBOX_* and legacy env vars.
Used by sandbox/config, sandbox/context, and quick.SkillRunner.
"""

import os
from typing import Optional


def parse_bool_env(
    key: str,
    default: bool,
    legacy_key: Optional[str] = None,
) -> bool:
    """
    Parse a boolean from environment variable.

    Accepts: true, false, 1, 0, yes, no, on, off (case-insensitive).
    Unknown values fall back to default.

    Args:
        key: Primary environment variable name (e.g. SKILLBOX_ALLOW_NETWORK)
        default: Default value if not set or invalid
        legacy_key: Optional legacy key to check if primary is not set

    Returns:
        Parsed boolean value
    """
    value = os.environ.get(key)
    if value is None and legacy_key:
        value = os.environ.get(legacy_key)
    if value is None:
        return default

    value_lower = value.lower().strip()
    if value_lower in ("true", "1", "yes", "on"):
        return True
    if value_lower in ("false", "0", "no", "off", ""):
        return False
    return default


def get_int_env(
    key: str,
    default: int,
    legacy_key: Optional[str] = None,
) -> int:
    """
    Parse an integer from environment variable.

    Args:
        key: Primary environment variable name (e.g. SKILLBOX_TIMEOUT_SECS)
        default: Default value if not set or invalid
        legacy_key: Optional legacy key to check if primary is not set

    Returns:
        Parsed integer value
    """
    value = os.environ.get(key)
    if value is None and legacy_key:
        value = os.environ.get(legacy_key)
    if value:
        try:
            return int(value)
        except ValueError:
            pass
    return default


# Convenience aliases for common Sandbox config keys
def get_timeout_from_env() -> int:
    """Execution timeout in seconds. SKILLBOX_TIMEOUT_SECS or EXECUTION_TIMEOUT."""
    return get_int_env("SKILLBOX_TIMEOUT_SECS", 120, "EXECUTION_TIMEOUT")


def get_memory_from_env() -> int:
    """Max memory in MB. SKILLBOX_MAX_MEMORY_MB or MAX_MEMORY_MB."""
    return get_int_env("SKILLBOX_MAX_MEMORY_MB", 512, "MAX_MEMORY_MB")
