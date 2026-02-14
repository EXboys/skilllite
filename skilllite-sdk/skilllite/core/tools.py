"""
Tool definitions and protocol adapters for Claude/OpenAI integration.

This is a CORE module - do not modify without explicit permission.
"""

import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional
import json

class ToolFormat(Enum):
    """Supported LLM tool formats."""
    CLAUDE = "claude"
    OPENAI = "openai"

@dataclass
class ToolDefinition:
    """Tool definition that can be sent to LLM APIs."""
    name: str
    description: str
    input_schema: Dict[str, Any]
    
    def to_claude_format(self) -> Dict[str, Any]:
        """Convert to Claude API format."""
        return {
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema
        }
    
    def to_openai_format(self) -> Dict[str, Any]:
        """Convert to OpenAI API format (function calling)."""
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema
            }
        }
    
    def to_format(self, format: ToolFormat) -> Dict[str, Any]:
        """Convert to specified format."""
        if format == ToolFormat.CLAUDE:
            return self.to_claude_format()
        elif format == ToolFormat.OPENAI:
            return self.to_openai_format()
        else:
            raise ValueError(f"Unsupported format: {format}")

def _unescape_json_string(s: str) -> str:
    """Unescape a JSON string value (handles \\n, \\\", \\\\, etc.)."""
    result = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            nxt = s[i + 1]
            if nxt == "n":
                result.append("\n")
            elif nxt == "t":
                result.append("\t")
            elif nxt == "r":
                result.append("\r")
            elif nxt == '"':
                result.append('"')
            elif nxt == "\\":
                result.append("\\")
            else:
                result.append(s[i : i + 2])
            i += 2
        else:
            result.append(s[i])
            i += 1
    return "".join(result)


def _parse_truncated_json_for_file_tools(arguments_str: str) -> Dict[str, Any]:
    """
    Attempt to extract file_path and content from truncated JSON.
    When LLM hits max_tokens (finish_reason: length), tool arguments may be cut off.
    This tries to recover file_path and content for write_output/write_file.
    """
    result: Dict[str, Any] = {}
    if not arguments_str or not isinstance(arguments_str, str):
        return result

    # Extract file_path: "file_path": "value"
    fp_match = re.search(r'"file_path"\s*:\s*"((?:[^"\\]|\\.)*)"', arguments_str)
    if fp_match:
        result["file_path"] = _unescape_json_string(fp_match.group(1))

    # Extract content: try complete JSON string first, then truncated
    content_match = re.search(r'"content"\s*:\s*"((?:[^"\\]|\\.)*)"', arguments_str, re.DOTALL)
    if content_match:
        raw = content_match.group(1)
        result["content"] = _unescape_json_string(raw)
    else:
        # Truncated: match from "content": " to end of string
        content_match = re.search(r'"content"\s*:\s*"(.*)$', arguments_str, re.DOTALL)
        if content_match:
            raw = content_match.group(1).rstrip()
            # Strip trailing "}" or " that may be from truncated JSON structure
            if raw.endswith('"}'):
                raw = raw[:-2]
            elif raw.endswith('"') and not raw.endswith('\\"'):
                raw = raw[:-1]
            result["content"] = _unescape_json_string(raw)

    return result


@dataclass
class ToolUseRequest:
    """Parsed tool use request from LLM response."""
    id: str
    name: str
    input: Dict[str, Any]
    
    @classmethod
    def from_claude_response(cls, content: Dict[str, Any]) -> Optional["ToolUseRequest"]:
        """Parse from Claude API tool_use content block."""
        if content.get("type") != "tool_use":
            return None
        
        return cls(
            id=content.get("id", ""),
            name=content.get("name", ""),
            input=content.get("input", {})
        )
    
    @classmethod
    def from_openai_response(cls, tool_call: Dict[str, Any]) -> Optional["ToolUseRequest"]:
        """Parse from OpenAI API tool_calls."""
        function = tool_call.get("function", {})
        arguments_str = function.get("arguments", "{}")
        if not isinstance(arguments_str, str):
            arguments_str = str(arguments_str) if arguments_str else "{}"

        try:
            arguments = json.loads(arguments_str)
        except json.JSONDecodeError:
            # LLM may hit max_tokens (finish_reason: length), truncating JSON.
            # Try to recover file_path/content for write_output, write_file.
            tool_name = function.get("name", "")
            if tool_name in ("write_output", "write_file"):
                arguments = _parse_truncated_json_for_file_tools(arguments_str)
            else:
                arguments = {}
        
        return cls(
            id=tool_call.get("id", ""),
            name=function.get("name", ""),
            input=arguments
        )
    
    @classmethod
    def parse_from_response(cls, response: Any, format: ToolFormat) -> List["ToolUseRequest"]:
        """
        Parse tool use requests from an LLM response.
        
        Args:
            response: The raw response from the LLM API
            format: The format of the response (Claude or OpenAI)
            
        Returns:
            List of ToolUseRequest objects
        """
        if format == ToolFormat.CLAUDE:
            return cls.parse_from_claude_response(response)
        elif format == ToolFormat.OPENAI:
            return cls.parse_from_openai_response(response)
        else:
            raise ValueError(f"Unsupported format: {format}")
    
    @classmethod
    def parse_from_openai_response(cls, response: Any) -> List["ToolUseRequest"]:
        """
        Parse tool use requests from an OpenAI-compatible LLM response.
        
        Works with any OpenAI-compatible provider:
        - OpenAI (GPT-4, GPT-3.5, etc.)
        - Azure OpenAI
        - Ollama
        - vLLM
        - LMStudio
        - DeepSeek
        - Qwen
        - Moonshot
        - etc.
        
        Args:
            response: The response from any OpenAI-compatible API
            
        Returns:
            List of ToolUseRequest objects
        """
        requests = []
        
        # Handle both object and dict responses
        if hasattr(response, 'choices'):
            message = response.choices[0].message
        elif isinstance(response, dict) and 'choices' in response:
            message = response['choices'][0].get('message', {})
        else:
            message = response
        
        # Get tool_calls from message
        if hasattr(message, 'tool_calls'):
            tool_calls = message.tool_calls or []
        elif isinstance(message, dict):
            tool_calls = message.get('tool_calls', []) or []
        else:
            tool_calls = []
        
        for tool_call in tool_calls:
            if isinstance(tool_call, dict):
                req = cls.from_openai_response(tool_call)
            else:
                # Handle object-style tool_call
                tool_name = getattr(tool_call.function, 'name', '')
                arguments_str = tool_call.function.arguments or "{}"
                if not isinstance(arguments_str, str):
                    arguments_str = str(arguments_str) if arguments_str else "{}"
                try:
                    arguments = json.loads(arguments_str)
                except (json.JSONDecodeError, AttributeError):
                    if tool_name in ("write_output", "write_file"):
                        arguments = _parse_truncated_json_for_file_tools(arguments_str)
                    else:
                        arguments = {}
                
                req = cls(
                    id=getattr(tool_call, 'id', ''),
                    name=tool_name,
                    input=arguments
                )
            if req:
                requests.append(req)
        
        return requests
    
    @classmethod
    def parse_from_claude_response(cls, response: Any) -> List["ToolUseRequest"]:
        """
        Parse tool use requests from Claude's native API response.
        
        Use this only if you're using the Anthropic SDK directly
        (not via OpenAI-compatible endpoint).
        
        Args:
            response: The response from Claude's native API
            
        Returns:
            List of ToolUseRequest objects
        """
        requests = []
        
        # Get content from response
        if hasattr(response, 'content'):
            content = response.content
        elif isinstance(response, dict):
            content = response.get('content', [])
        else:
            content = []
        
        for block in content:
            if isinstance(block, dict):
                req = cls.from_claude_response(block)
            elif hasattr(block, 'type') and block.type == 'tool_use':
                req = cls(
                    id=getattr(block, 'id', ''),
                    name=getattr(block, 'name', ''),
                    input=block.input if isinstance(block.input, dict) else {}
                )
            else:
                continue
            
            if req:
                requests.append(req)
        
        return requests

@dataclass
class ToolResult:
    """Result of a tool execution to send back to the LLM."""
    tool_use_id: str
    content: str
    is_error: bool = False
    
    @classmethod
    def success(cls, tool_use_id: str, content: Any) -> "ToolResult":
        """Create a successful result."""
        if not isinstance(content, str):
            content = json.dumps(content)
        return cls(tool_use_id=tool_use_id, content=content, is_error=False)
    
    @classmethod
    def error(cls, tool_use_id: str, error: str) -> "ToolResult":
        """Create an error result."""
        return cls(tool_use_id=tool_use_id, content=error, is_error=True)
    
    def to_claude_format(self) -> Dict[str, Any]:
        """Convert to Claude API format."""
        return {
            "type": "tool_result",
            "tool_use_id": self.tool_use_id,
            "content": self.content,
            "is_error": self.is_error
        }
    
    def to_openai_format(self) -> Dict[str, Any]:
        """Convert to OpenAI API format."""
        return {
            "role": "tool",
            "tool_call_id": self.tool_use_id,
            "content": self.content
        }
    
    def to_format(self, format: ToolFormat) -> Dict[str, Any]:
        """Convert to specified format."""
        if format == ToolFormat.CLAUDE:
            return self.to_claude_format()
        elif format == ToolFormat.OPENAI:
            return self.to_openai_format()
        else:
            raise ValueError(f"Unsupported format: {format}")
