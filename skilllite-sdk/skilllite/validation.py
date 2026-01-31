"""
Validation utilities for Agent Skills specification.

This module provides validation functions for skill names and descriptions
according to the Skills specification.
"""

import re
import warnings
from pathlib import Path
from typing import List, Optional, Tuple


class SkillNameValidationError(ValueError):
    """Raised when skill name doesn't conform to Agent Skills specification."""
    pass


class SkillNameValidationWarning(UserWarning):
    """Warning for skill name validation issues."""
    pass


def validate_skill_name(name: str, skill_dir: Optional[Path] = None) -> Tuple[bool, List[str]]:
    """
    Validate skill name according to Agent Skills specification.
    
    Rules from Skills specification:
    - Must be 1-64 characters
    - May only contain lowercase alphanumeric characters and hyphens (a-z, 0-9, -)
    - Must not start or end with hyphen (-)
    - Must not contain consecutive hyphens (--)
    - Must match the parent directory name (if skill_dir provided)
    
    Args:
        name: The skill name to validate
        skill_dir: Optional path to skill directory for directory name matching
        
    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    errors = []
    
    # Check length
    if not name:
        errors.append("Skill name cannot be empty")
    elif len(name) > 64:
        errors.append(f"Skill name exceeds 64 characters (got {len(name)})")
    
    # Check character set (lowercase alphanumeric and hyphens only)
    if name and not re.match(r'^[a-z0-9-]+$', name):
        invalid_chars = set(re.findall(r'[^a-z0-9-]', name))
        if any(c.isupper() for c in name):
            errors.append("Skill name must be lowercase (found uppercase characters)")
        else:
            errors.append(f"Skill name contains invalid characters: {invalid_chars}")
    
    # Check hyphen rules
    if name:
        if name.startswith('-'):
            errors.append("Skill name must not start with a hyphen")
        if name.endswith('-'):
            errors.append("Skill name must not end with a hyphen")
        if '--' in name:
            errors.append("Skill name must not contain consecutive hyphens (--)")
    
    # Check directory name match
    if skill_dir and name:
        dir_name = skill_dir.name
        if dir_name != name:
            errors.append(f"Skill name '{name}' must match directory name '{dir_name}'")
    
    return len(errors) == 0, errors


def validate_skill_name_strict(name: str, skill_dir: Optional[Path] = None) -> None:
    """
    Validate skill name and raise exception if invalid.
    
    Args:
        name: The skill name to validate
        skill_dir: Optional path to skill directory
        
    Raises:
        SkillNameValidationError: If name is invalid
    """
    is_valid, errors = validate_skill_name(name, skill_dir)
    if not is_valid:
        raise SkillNameValidationError(
            f"Invalid skill name '{name}': " + "; ".join(errors)
        )


def validate_skill_name_warn(name: str, skill_dir: Optional[Path] = None) -> bool:
    """
    Validate skill name and emit warnings if invalid.
    
    Args:
        name: The skill name to validate
        skill_dir: Optional path to skill directory
        
    Returns:
        True if valid, False otherwise (with warnings emitted)
    """
    is_valid, errors = validate_skill_name(name, skill_dir)
    if not is_valid:
        for error in errors:
            warnings.warn(
                f"Skill name validation: {error}",
                SkillNameValidationWarning,
                stacklevel=3
            )
    return is_valid


def validate_description(description: Optional[str]) -> Tuple[bool, List[str]]:
    """
    Validate skill description according to Agent Skills specification.
    
    Rules:
    - Must be 1-1024 characters
    - Should describe what the skill does and when to use it
    
    Args:
        description: The skill description to validate
        
    Returns:
        Tuple of (is_valid, list_of_errors)
    """
    errors = []
    
    if not description:
        errors.append("Skill description is required")
    elif len(description) > 1024:
        errors.append(f"Skill description exceeds 1024 characters (got {len(description)})")
    
    return len(errors) == 0, errors
