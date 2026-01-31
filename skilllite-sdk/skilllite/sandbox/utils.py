"""
Common utilities for sandbox implementations.

This module provides shared functionality used by different executor
implementations, reducing code duplication.
"""

from typing import Any, Dict, List


# Default positional argument keys that should be treated as positional args
DEFAULT_POSITIONAL_KEYS = {"skill_name", "skill-name", "name", "input", "file", "filename"}


def convert_json_to_cli_args(
    input_data: Dict[str, Any],
    positional_keys: set = None
) -> List[str]:
    """
    Convert JSON input data to command line arguments list.
    
    This handles the conversion of JSON parameters to CLI format:
    - Positional args: keys like "skill_name" or "skill-name" become positional values
    - Named args: keys like "path" become "--path value"
    - Boolean flags: true becomes "--flag", false is omitted
    - Arrays: become comma-separated values
    
    Args:
        input_data: JSON input data from LLM
        positional_keys: Set of keys to treat as positional arguments.
                        Defaults to DEFAULT_POSITIONAL_KEYS.
        
    Returns:
        List of command line arguments
        
    Example:
        >>> convert_json_to_cli_args({"name": "test", "verbose": True, "count": 5})
        ['test', '--verbose', '--count', '5']
    """
    if positional_keys is None:
        positional_keys = DEFAULT_POSITIONAL_KEYS
    
    args_list = []
    
    # First, handle positional arguments
    for key in positional_keys:
        if key in input_data:
            value = input_data[key]
            if isinstance(value, str):
                args_list.append(value)
            break
    
    # Then handle named arguments
    for key, value in input_data.items():
        # Skip positional args already handled
        normalized_key = key.replace("-", "_")
        if normalized_key in {k.replace("-", "_") for k in positional_keys}:
            continue
        
        # Convert key to CLI format (e.g., "skill_name" -> "--skill-name")
        cli_key = f"--{key.replace('_', '-')}"
        
        if isinstance(value, bool):
            # Boolean flags: only add if True
            if value:
                args_list.append(cli_key)
        elif isinstance(value, list):
            # Arrays become comma-separated
            if value:
                args_list.append(cli_key)
                args_list.append(",".join(str(v) for v in value))
        elif value is not None:
            # Regular values
            args_list.append(cli_key)
            args_list.append(str(value))
    
    return args_list
