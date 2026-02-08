"""
Common utilities for sandbox implementations.

This module provides shared functionality used by different executor
implementations, reducing code duplication.
"""

import json
from typing import Any, Dict, List, Optional


# Default positional argument keys that should be treated as positional args
DEFAULT_POSITIONAL_KEYS = {"skill_name", "skill-name", "name", "input", "file", "filename"}


def extract_json_from_output(output: str, strategy: str = "auto") -> Optional[Any]:
    """
    Extract JSON from output that may contain log lines or other text.
    
    Supports multiple extraction strategies:
    - "auto": Try all strategies in order
    - "full": Parse entire output as JSON
    - "line": Look for JSON in individual lines
    - "brace": Find JSON object by matching braces
    
    Args:
        output: Raw output from subprocess (may contain logs, errors, etc.)
        strategy: Extraction strategy to use (default: "auto")
        
    Returns:
        Parsed JSON object if found, None otherwise
        
    Example:
        >>> extract_json_from_output("[INFO] Starting...\\n{\"result\": \"success\"}")
        {'result': 'success'}
    """
    if not output:
        return None
    
    # Strategy: full - parse entire output as JSON
    if strategy in ("auto", "full"):
        try:
            return json.loads(output.strip())
        except json.JSONDecodeError:
            if strategy == "full":
                return None
    
    # Strategy: line - look for JSON in individual lines
    if strategy in ("auto", "line"):
        for line in output.split('\n'):
            line = line.strip()
            if line.startswith('{') and line.endswith('}'):
                try:
                    data = json.loads(line)
                    if isinstance(data, dict):
                        return data
                except json.JSONDecodeError:
                    continue
    
    # Strategy: brace - find JSON object by matching braces
    # This handles cases where JSON contains newlines (like \n in strings)
    if strategy in ("auto", "brace"):
        brace_start = output.rfind('{')
        if brace_start != -1:
            brace_end = output.rfind('}')
            if brace_end != -1 and brace_end >= brace_start:
                json_str = output[brace_start:brace_end + 1]
                try:
                    return json.loads(json_str)
                except json.JSONDecodeError:
                    pass
    
    return None


def format_sandbox_error(error_msg: str) -> str:
    """
    Format sandbox restriction errors into user-friendly messages.
    
    This function recognizes common sandbox error patterns and converts
    them into more user-friendly messages, hiding technical tracebacks.
    
    Args:
        error_msg: Raw error message from subprocess
        
    Returns:
        Formatted error message
        
    Example:
        >>> format_sandbox_error("BlockingIOError: Operation not permitted")
        '🔒 Sandbox blocked process creation (fork/exec not allowed)\\n\\n💡 This skill requires operations that are blocked by the sandbox for security reasons.'
    """
    # Common sandbox restriction patterns and their friendly messages
    sandbox_errors = {
        "BlockingIOError": "🔒 Sandbox blocked process creation (fork/exec not allowed)",
        "Resource temporarily unavailable": "🔒 Sandbox blocked system resource access",
        "Operation not permitted": "🔒 Sandbox blocked this operation",
        "Permission denied": "🔒 Sandbox denied file/resource access",
        "sandbox-exec": "🔒 Sandbox restriction triggered",
        "seccomp": "🔒 System call blocked by sandbox",
        "namespace": "🔒 Namespace isolation restriction",
    }
    
    # Check for patterns in error message
    for pattern, friendly_msg in sandbox_errors.items():
        if pattern in error_msg:
            # Return only the friendly message, hide the traceback
            return f"{friendly_msg}\n\n💡 This skill requires operations that are blocked by the sandbox for security reasons."
    
    # No pattern matched, return original error message
    return error_msg


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
