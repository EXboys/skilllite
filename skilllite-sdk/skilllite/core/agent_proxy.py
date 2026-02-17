"""
Thin proxy for AgenticLoop — delegates to skillbox agent-rpc.

Replaces the former Python AgenticLoop implementation.
"""

from typing import Any, Callable, Dict, List, Optional

from ..sandbox.skillbox.agent_rpc_client import agent_chat


class ApiFormat:
    """API format enum (backward compatibility)."""
    OPENAI = "openai"
    CLAUDE_NATIVE = "claude_native"


class _Message:
    def __init__(self, content: str = ""):
        self.content = content


class _Choice:
    def __init__(self, message: _Message):
        self.message = message


class _Response:
    """Minimal response object matching OpenAI-style response.choices[0].message.content."""
    def __init__(self, content: str):
        self.choices = [_Choice(_Message(content))]


class AgenticLoop:
    """
    Proxy that delegates to skillbox agent-rpc.
    Replaces the former Python implementation.
    """

    def __init__(
        self,
        manager: Any,
        client: Any,
        model: str,
        system_prompt: Optional[str] = None,
        max_iterations: int = 50,
        max_tool_calls_per_task: int = 30,
        api_format: str = "openai",
        custom_tool_handler: Optional[Callable] = None,
        custom_tools: Optional[List[Dict[str, Any]]] = None,
        enable_task_planning: bool = True,
        verbose: bool = True,
        confirmation_callback: Optional[Callable[[str, str], bool]] = None,
        **kwargs: Any,
    ):
        self._manager = manager
        self._client = client
        self._model = model
        self._max_iterations = max_iterations
        self._confirmation_callback = confirmation_callback
        from pathlib import Path
        skills_dir = getattr(manager, "skills_dir", ".skills")
        self._workspace = str(Path(skills_dir).resolve().parent)
        # API key from client if available
        self._api_key = getattr(client, "api_key", None) or ""
        self._api_base = getattr(client, "base_url", None) or ""

    def run(
        self,
        user_message: str,
        timeout: Optional[int] = None,
        stream_callback: Optional[Callable[[str], None]] = None,
    ) -> _Response:
        """Run agent loop via skillbox agent-rpc."""
        import os
        api_key = self._api_key or os.environ.get("OPENAI_API_KEY") or os.environ.get("API_KEY")
        api_base = self._api_base or os.environ.get("OPENAI_API_BASE") or os.environ.get("BASE_URL")
        result = agent_chat(
            user_message,
            session_key="default",
            workspace=self._workspace,
            model=self._model,
            api_base=api_base,
            api_key=api_key,
            max_iterations=self._max_iterations,
            stream_callback=stream_callback,
            confirmation_callback=self._confirmation_callback,
        )
        return _Response(result)


# Alias for backward compatibility
AgenticLoopClaudeNative = AgenticLoop
