"""
Built-in tools for SkillLite SDK.

This module provides commonly needed tools like file operations
that can be used with create_enhanced_agentic_loop.
"""

import json
from pathlib import Path
from typing import Any, Dict, List, Optional


def get_builtin_file_tools() -> List[Dict[str, Any]]:
    """
    Get built-in file operation tools.
    
    Returns:
        List of tool definitions in OpenAI-compatible format
    """
    return [
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the content of a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to read (relative or absolute)"
                        }
                    },
                    "required": ["file_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write content to a file. Creates the file if it doesn't exist, overwrites if it does.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to write (relative or absolute)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["file_path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List files and directories in a given path",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "directory_path": {
                            "type": "string",
                            "description": "Path to the directory to list (relative or absolute)"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Whether to list recursively (default: false)",
                            "default": False
                        }
                    },
                    "required": ["directory_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "file_exists",
                "description": "Check if a file or directory exists",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to check (relative or absolute)"
                        }
                    },
                    "required": ["file_path"]
                }
            }
        }
    ]


def execute_builtin_file_tool(tool_name: str, tool_input: Dict[str, Any]) -> str:
    """
    Execute a built-in file operation tool.
    
    Args:
        tool_name: Name of the tool to execute
        tool_input: Input parameters for the tool
        
    Returns:
        Result of the tool execution as a string
        
    Raises:
        ValueError: If tool_name is not recognized
        Exception: If tool execution fails
    """
    try:
        if tool_name == "read_file":
            return _read_file(tool_input["file_path"])
        elif tool_name == "write_file":
            return _write_file(tool_input["file_path"], tool_input["content"])
        elif tool_name == "list_directory":
            recursive = tool_input.get("recursive", False)
            return _list_directory(tool_input["directory_path"], recursive)
        elif tool_name == "file_exists":
            return _file_exists(tool_input["file_path"])
        else:
            raise ValueError(f"Unknown built-in tool: {tool_name}")
    except KeyError as e:
        return f"Error: Missing required parameter: {e}"
    except Exception as e:
        return f"Error executing {tool_name}: {str(e)}"


def _read_file(file_path: str) -> str:
    """Read file content."""
    path = Path(file_path)
    
    if not path.exists():
        return f"Error: File not found: {file_path}"
    
    if not path.is_file():
        return f"Error: Path is not a file: {file_path}"
    
    try:
        content = path.read_text(encoding="utf-8")
        return f"Successfully read file: {file_path}\n\nContent:\n{content}"
    except UnicodeDecodeError:
        # Try reading as binary and return info
        size = path.stat().st_size
        return f"File {file_path} appears to be binary (size: {size} bytes). Cannot display content."
    except Exception as e:
        return f"Error reading file {file_path}: {str(e)}"


def _write_file(file_path: str, content: str) -> str:
    """Write content to file."""
    path = Path(file_path)
    
    try:
        # Create parent directories if they don't exist
        path.parent.mkdir(parents=True, exist_ok=True)
        
        # Write the file
        path.write_text(content, encoding="utf-8")
        
        return f"Successfully wrote to file: {file_path} ({len(content)} characters)"
    except Exception as e:
        return f"Error writing to file {file_path}: {str(e)}"


def _list_directory(directory_path: str, recursive: bool = False) -> str:
    """List directory contents."""
    path = Path(directory_path)
    
    if not path.exists():
        return f"Error: Directory not found: {directory_path}"
    
    if not path.is_dir():
        return f"Error: Path is not a directory: {directory_path}"
    
    try:
        items = []
        
        if recursive:
            # Recursive listing
            for item in path.rglob("*"):
                rel_path = item.relative_to(path)
                item_type = "dir" if item.is_dir() else "file"
                items.append(f"{item_type}: {rel_path}")
        else:
            # Non-recursive listing
            for item in path.iterdir():
                item_type = "dir" if item.is_dir() else "file"
                items.append(f"{item_type}: {item.name}")
        
        if not items:
            return f"Directory is empty: {directory_path}"
        
        items.sort()
        result = f"Contents of {directory_path}:\n" + "\n".join(items)
        return result
    except Exception as e:
        return f"Error listing directory {directory_path}: {str(e)}"


def _file_exists(file_path: str) -> str:
    """Check if file exists."""
    path = Path(file_path)
    
    if path.exists():
        if path.is_file():
            size = path.stat().st_size
            return f"File exists: {file_path} (size: {size} bytes)"
        elif path.is_dir():
            return f"Directory exists: {file_path}"
        else:
            return f"Path exists but is neither file nor directory: {file_path}"
    else:
        return f"Path does not exist: {file_path}"


def create_builtin_tool_executor():
    """
    Create an executor function for built-in tools.
    
    Returns:
        Executor function that can be passed to create_enhanced_agentic_loop
    """
    builtin_tool_names = {"read_file", "write_file", "list_directory", "file_exists"}
    
    def executor(tool_input: Dict[str, Any]) -> str:
        """Execute built-in tools."""
        tool_name = tool_input.get("tool_name")
        
        if tool_name not in builtin_tool_names:
            raise ValueError(f"Not a built-in tool: {tool_name}")
        
        return execute_builtin_file_tool(tool_name, tool_input)
    
    return executor
