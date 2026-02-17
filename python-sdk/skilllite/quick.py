"""
SkillLite Quick Start - Minimal wrapper for running Skills with one line of code.

Delegates to skillbox agent-rpc (Rust) for agent loop, tools, and execution.
"""

import os
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

from .logger import get_logger
from .sandbox.core.agent_rpc_client import agent_chat


def load_env(env_file: Optional[Union[str, Path]] = None) -> Dict[str, str]:
    """
    Load .env file into environment variables.
    """
    if env_file is None:
        env_file = Path.cwd() / ".env"
    else:
        env_file = Path(env_file)

    loaded = {}
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                key, value = line.split("=", 1)
                key, value = key.strip(), value.strip()
                if value:
                    os.environ.setdefault(key, value)
                    loaded[key] = value
    return loaded


class SkillRunner:
    """
    Minimal Skill Runner - Delegates to skillbox agent-rpc.
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        skills_dir: Optional[Union[str, Path]] = None,
        env_file: Optional[Union[str, Path]] = None,
        max_iterations: int = 50,
        verbose: bool = False,
        enable_builtin_tools: bool = True,
        confirmation_callback: Optional[Any] = None,
        **kwargs: Any,
    ):
        load_env(env_file)
        self.base_url = base_url or os.environ.get("BASE_URL") or os.environ.get("OPENAI_API_BASE")
        self.api_key = api_key or os.environ.get("API_KEY") or os.environ.get("OPENAI_API_KEY")
        self.model = model or os.environ.get("MODEL", "deepseek-chat")
        self.skills_dir = str(skills_dir or "./.skills")
        self.max_iterations = max_iterations
        self.verbose = verbose
        self.confirmation_callback = confirmation_callback
        self._logger = get_logger("skilllite.quick", verbose=verbose)
        self._manager = None
        # Ignored (delegated to Rust): include_full_instructions, context_mode, etc.
        if kwargs:
            self._logger.debug("Ignored kwargs (Rust handles): %s", list(kwargs.keys()))

    @property
    def manager(self):
        """SkillManager instance (lazy, for skill_names() etc.)."""
        if self._manager is None:
            from .core import SkillManager
            self._manager = SkillManager(skills_dir=self.skills_dir)
            if self.verbose:
                self._logger.info("📦 Loaded Skills: %s", self._manager.skill_names())
        return self._manager

    @property
    def workspace(self) -> str:
        return str(Path(self.skills_dir).resolve().parent)

    def run(
        self,
        user_message: str,
        stream: bool = False,
        stream_callback: Optional[Any] = None,
    ) -> str:
        """Run skill via skillbox agent-rpc."""
        if self.verbose:
            self._logger.info("👤 User: %s", user_message)

        effective_callback = stream_callback
        if effective_callback is None and (stream or self.verbose):
            import sys
            def _default_stream(chunk: str) -> None:
                sys.stdout.write(chunk)
                sys.stdout.flush()
            effective_callback = _default_stream

        result = agent_chat(
            user_message,
            session_key="default",
            workspace=self.workspace,
            model=self.model,
            api_base=self.base_url,
            api_key=self.api_key,
            max_iterations=self.max_iterations,
            stream_callback=effective_callback,
            confirmation_callback=self.confirmation_callback,
        )

        if self.verbose:
            self._logger.info("🤖 Assistant: %s", result[:200] + "..." if len(result) > 200 else result)
        return result

    def run_with_details(self, user_message: str) -> Dict[str, Any]:
        """Run and return details (content, iterations). Events collected from stream."""
        content_parts: List[str] = []
        iterations = 0
        tool_calls: List[Dict[str, Any]] = []

        def _collect(event: Dict[str, Any]) -> None:
            nonlocal iterations
            ev = event.get("event")
            data = event.get("data") or {}
            if ev == "text" or ev == "text_chunk":
                content_parts.append(data.get("text", ""))
            elif ev == "tool_call":
                tool_calls.append({"name": data.get("name"), "arguments": data.get("arguments"), "result": None})
                iterations += 1
            elif ev == "tool_result":
                for entry in reversed(tool_calls):
                    if entry.get("result") is None:
                        entry["result"] = data.get("result", "")
                        break

        from .sandbox.core.agent_rpc_client import agent_chat_stream
        final = ""
        for ev in agent_chat_stream(
            user_message,
            session_key="default",
            workspace=self.workspace,
            model=self.model,
            api_base=self.base_url,
            api_key=self.api_key,
            max_iterations=self.max_iterations,
            confirmation_callback=self.confirmation_callback,
        ):
            _collect(ev)
            if ev.get("event") == "done":
                final = ev.get("data", {}).get("response", "".join(content_parts))
            elif ev.get("event") == "error":
                raise RuntimeError(ev.get("data", {}).get("message", "Unknown error"))

        return {
            "content": final or "".join(content_parts),
            "iterations": iterations,
            "tool_calls": tool_calls,
            "final_response": {"choices": [{"message": {"content": final}}]},
        }


def quick_run(
    user_message: str,
    skills_dir: Optional[str] = None,
    verbose: bool = False,
    **kwargs: Any,
) -> str:
    """One-line run via skillbox agent-rpc."""
    if skills_dir:
        kwargs["skills_dir"] = skills_dir
    kwargs["verbose"] = verbose
    runner = SkillRunner(**kwargs)
    return runner.run(user_message)
