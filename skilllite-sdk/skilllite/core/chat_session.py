"""
Chat Session - Persistent conversation with transcript and memory.

Integrates session/transcript/memory from skillbox Rust core with Python
AgenticLoop. Supports multi-turn dialogue with memory retrieval.
"""

import os
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional, Callable

from ..logger import get_logger

if False:  # TYPE_CHECKING
    from .manager import SkillManager


def _ensure_chat_client():
    """Get SkillboxIPCClient for executor RPC. Requires skillbox built with --features executor."""
    from ..sandbox.skillbox import find_binary
    from ..sandbox.skillbox.ipc_client import SkillboxIPCClient

    binary = find_binary()
    if not binary:
        raise RuntimeError("skillbox binary not found. Install with: skilllite install")
    return SkillboxIPCClient(binary_path=binary)


def _transcript_entries_to_messages(entries: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Convert transcript entries to OpenAI message format.

    Compaction-aware: if a ``compaction`` entry exists, only use its summary
    (as a system message) plus entries that come *after* it.  This keeps
    context small while preserving important information.
    """
    # Find the last compaction entry (if any)
    last_compaction_idx = -1
    for i, e in enumerate(entries):
        if e.get("type") == "compaction":
            last_compaction_idx = i

    messages: List[Dict[str, Any]] = []

    if last_compaction_idx >= 0:
        # Inject compaction summary as a system message
        compaction = entries[last_compaction_idx]
        summary = compaction.get("summary") or ""
        if summary:
            messages.append({
                "role": "system",
                "content": f"[Compacted conversation summary]\n{summary}",
            })
        # Only process entries after the compaction
        remaining = entries[last_compaction_idx + 1:]
    else:
        remaining = entries

    for e in remaining:
        if e.get("type") != "message":
            continue
        role = e.get("role")
        content = e.get("content") or ""
        tool_calls = e.get("tool_calls")
        if role not in ("user", "assistant", "system"):
            continue
        msg: Dict[str, Any] = {"role": role, "content": content}
        if tool_calls:
            msg["tool_calls"] = tool_calls
        messages.append(msg)
    return messages


def _message_to_transcript_entry(role: str, content: str, parent_id: Optional[str] = None) -> Dict[str, Any]:
    """Create transcript entry from a simple message."""
    return {
        "type": "message",
        "id": f"e_{uuid.uuid4().hex[:12]}",
        "parent_id": parent_id,
        "role": role,
        "content": content,
    }


_COMPACTION_PROMPT = """Summarize the following conversation concisely. Keep:
- Key decisions and conclusions
- Important facts, names, numbers
- User preferences and instructions
- Any unresolved questions or tasks

Be concise but preserve essential information. Output the summary only, no preamble."""


class ChatSession:
    """
    Persistent chat session with transcript history and memory.

    Uses skillbox IPC for session/transcript/memory. Integrates with
    AgenticLoop for LLM execution. Registers memory_search and memory_write
    as tools for the LLM.

    Supports auto-compaction: when message count exceeds
    ``compaction_threshold`` (default 30), older messages are summarized
    via a silent LLM call and a ``compaction`` entry is written to the
    transcript.  Subsequent reads only use the summary + post-compaction
    messages, keeping context small.
    """

    # Number of *message* entries (user+assistant) that trigger compaction
    COMPACTION_THRESHOLD = 30
    # How many recent messages to keep after compaction
    COMPACTION_KEEP_RECENT = 10

    def __init__(
        self,
        manager: "SkillManager",
        client: Any,
        model: str,
        session_key: str = "main",
        workspace_path: Optional[str] = None,
        system_prompt: Optional[str] = None,
        max_iterations: int = 50,
        max_tool_calls_per_task: int = 30,
        enable_builtin_tools: bool = True,
        enable_memory_tools: bool = True,
        verbose: bool = True,
        confirmation_callback: Optional[Callable[[str, str], bool]] = None,
    ):
        self.manager = manager
        self.client = client
        self.model = model
        self.session_key = session_key
        self.workspace_path = workspace_path or str(Path.home() / ".skilllite" / "chat")
        self.system_prompt = system_prompt
        self.max_iterations = max_iterations
        self.max_tool_calls_per_task = max_tool_calls_per_task
        self.enable_builtin_tools = enable_builtin_tools
        self.enable_memory_tools = enable_memory_tools
        self.verbose = verbose
        self.confirmation_callback = confirmation_callback
        self._logger = get_logger("skilllite.core.chat_session", verbose=verbose)
        self._ipc_client = None  # Lazy init

    def _get_ipc(self):
        if self._ipc_client is None:
            self._ipc_client = _ensure_chat_client()
        return self._ipc_client

    def _ensure_session(self) -> str:
        """Create or get session, return session_id."""
        ipc = self._get_ipc()
        r = ipc.session_create(session_key=self.session_key, workspace_path=self.workspace_path)
        session_id = r.get("session_id", "default")
        session_key = r.get("session_key", self.session_key)
        ipc.transcript_ensure(
            session_key=session_key,
            session_id=session_id,
            workspace_path=self.workspace_path,
        )
        return session_id

    def _read_transcript(self) -> List[Dict[str, Any]]:
        """Read transcript entries."""
        ipc = self._get_ipc()
        return ipc.transcript_read(session_key=self.session_key, workspace_path=self.workspace_path)

    def _append_transcript(self, entry: Dict[str, Any]) -> None:
        """Append entry to transcript."""
        ipc = self._get_ipc()
        ipc.transcript_append(
            session_key=self.session_key,
            entry=entry,
            workspace_path=self.workspace_path,
        )

    def _check_and_compact(self, entries: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """Check if compaction is needed and perform it if so.

        Returns the (possibly updated) entries list.
        """
        # Count message entries only
        msg_entries = [e for e in entries if e.get("type") == "message"]
        if len(msg_entries) < self.COMPACTION_THRESHOLD:
            return entries

        self._logger.info(
            "[Compaction] %d messages exceed threshold (%d), compacting...",
            len(msg_entries), self.COMPACTION_THRESHOLD,
        )

        # Split: messages to summarize vs. messages to keep
        to_summarize = msg_entries[:-self.COMPACTION_KEEP_RECENT]
        kept = msg_entries[-self.COMPACTION_KEEP_RECENT:]

        # Build conversation text for summarization
        conv_lines = []
        for m in to_summarize:
            role = m.get("role", "unknown")
            content = m.get("content", "")
            # Truncate very long individual messages
            if len(content) > 500:
                content = content[:500] + "..."
            conv_lines.append(f"{role}: {content}")
        conversation_text = "\n".join(conv_lines)

        # Call LLM for summary (silent, no tools)
        try:
            response = self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": _COMPACTION_PROMPT},
                    {"role": "user", "content": conversation_text},
                ],
                max_tokens=1024,
            )
            summary = response.choices[0].message.content or ""
        except Exception as e:
            self._logger.warning("[Compaction] LLM call failed: %s, skipping", e)
            return entries

        # Write compaction entry to transcript
        first_kept_id = kept[0].get("id", "") if kept else ""
        compaction_entry = {
            "type": "compaction",
            "id": f"c_{uuid.uuid4().hex[:12]}",
            "parent_id": to_summarize[-1].get("id") if to_summarize else None,
            "first_kept_entry_id": first_kept_id,
            "tokens_before": len(conversation_text),  # approximate
            "summary": summary,
        }
        self._append_transcript(compaction_entry)

        # Update session compaction count
        try:
            ipc = self._get_ipc()
            session_info = ipc.session_get(
                session_key=self.session_key,
                workspace_path=self.workspace_path,
            )
            count = session_info.get("compaction_count", 0) + 1
            ipc.session_update(
                session_key=self.session_key,
                workspace_path=self.workspace_path,
                compaction_count=count,
            )
        except Exception:
            pass  # non-critical

        self._logger.info("[Compaction] Done. Summarized %d messages.", len(to_summarize))

        # Re-read to get updated entries with the compaction
        return self._read_transcript()

    def summarize_for_memory(self) -> Optional[str]:
        """Summarize the current session transcript and write to memory.

        Used for memory flush before /clear or session switch.
        Returns the summary text, or None if nothing to summarize.
        """
        entries = self._read_transcript()
        msg_entries = [e for e in entries if e.get("type") == "message"]
        if len(msg_entries) < 2:
            return None  # nothing worth summarizing

        # Build conversation text
        conv_lines = []
        for m in msg_entries:
            role = m.get("role", "unknown")
            content = m.get("content", "")
            if len(content) > 500:
                content = content[:500] + "..."
            conv_lines.append(f"{role}: {content}")
        conversation_text = "\n".join(conv_lines)

        # Call LLM for summary
        flush_prompt = (
            "Summarize the key information from this conversation that should be "
            "remembered for future sessions. Focus on: user preferences, decisions, "
            "important facts, and any pending tasks. Be concise."
        )
        try:
            response = self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {"role": "system", "content": flush_prompt},
                    {"role": "user", "content": conversation_text},
                ],
                max_tokens=1024,
            )
            summary = response.choices[0].message.content or ""
        except Exception as e:
            self._logger.warning("[Memory Flush] LLM call failed: %s", e)
            return None

        if not summary.strip():
            return None

        # Write to memory via IPC
        try:
            import time
            ipc = self._get_ipc()
            rel_path = f"session_summaries/{self.session_key}.md"
            header = f"# Session: {self.session_key}\n_Flushed: {time.strftime('%Y-%m-%d %H:%M')}_\n\n"
            ipc.memory_write(
                rel_path=rel_path,
                content=header + summary + "\n",
                workspace_path=self.workspace_path,
                append=False,
            )
            self._logger.info("[Memory Flush] Saved to %s (%d chars)", rel_path, len(summary))
        except Exception as e:
            self._logger.warning("[Memory Flush] Write failed: %s", e)

        return summary

    def _build_memory_context(self, user_message: str, limit: int = 5) -> str:
        """Get relevant memory context for the user message."""
        from ..extensions.memory import build_memory_context

        return build_memory_context(
            ipc=self._get_ipc(),
            workspace_path=self.workspace_path,
            user_message=user_message,
            limit=limit,
        )

    def _build_planner_context(self, history: List[Dict[str, Any]], keep_last: int = 6, max_per_msg: int = 600) -> str:
        """Build conversation context for task planner (e.g. when user says '继续未完成任务')."""
        if not history:
            return ""
        recent = history[-keep_last:] if len(history) > keep_last else history
        lines = []
        for m in recent:
            role = m.get("role", "unknown")
            content = m.get("content") or ""
            if content and role in ("user", "assistant", "system"):
                if len(content) > max_per_msg:
                    content = content[:max_per_msg] + "..."
                lines.append(f"[{role}]: {content}")
        return "\n".join(lines) if lines else ""

    def _build_custom_tools_and_executor(self):
        """Build custom tools via extensions (ToolRegistry + centralized registration)."""
        from .tool_registry import ToolRegistry
        from ..builtin_tools import resolve_output_dir
        from ..extensions import ExtensionsContext, register_extensions

        registry = ToolRegistry()
        workspace_root = Path(self.workspace_path).resolve()
        output_root = resolve_output_dir(workspace_root)
        output_root.mkdir(parents=True, exist_ok=True)

        ctx = ExtensionsContext(
            workspace_root=workspace_root,
            output_root=output_root,
            workspace_path=self.workspace_path,
            confirmation_callback=self.confirmation_callback,
        )
        register_extensions(
            registry,
            ctx,
            enable_file_tools=self.enable_builtin_tools,
            enable_memory_tools=self.enable_memory_tools,
        )

        def combined_executor(tool_input: Dict[str, Any]) -> str:
            return registry.execute(tool_input.get("tool_name", ""), tool_input)

        return registry.get_tool_definitions(), combined_executor

    def run_turn(self, user_message: str) -> str:
        """
        Run one turn of conversation.

        1. Ensure session
        2. Read transcript history
        3. Search memory for context
        4. Build messages and run AgenticLoop
        5. Append user + assistant messages to transcript

        Returns:
            Assistant response text
        """
        self._ensure_session()

        # Read transcript and auto-compact if needed
        entries = self._read_transcript()
        entries = self._check_and_compact(entries)
        history = _transcript_entries_to_messages(entries)

        # Memory context
        mem_ctx = self._build_memory_context(user_message)
        system_parts = []
        if self.system_prompt:
            system_parts.append(self.system_prompt)
        if mem_ctx:
            system_parts.append(mem_ctx)
        system_content = "\n\n".join(system_parts) if system_parts else None

        # Build messages for this turn
        messages: List[Dict[str, Any]] = []
        if system_content:
            messages.append({"role": "system", "content": system_content})

        # History is already compaction-aware (only post-compaction messages
        # plus the summary).  Apply a safety cap in case compaction didn't
        # trigger yet or just completed.
        max_history = 40
        if len(history) > max_history:
            history = history[-max_history:]
        messages.extend(history)
        messages.append({"role": "user", "content": user_message})

        # Build conversation context for task planner (e.g. "继续未完成任务" needs context)
        conv_ctx = self._build_planner_context(history)

        # Append user message first (plan and assistant will follow)
        last_id = entries[-1].get("id") if entries else None
        user_entry = _message_to_transcript_entry("user", user_message, last_id)
        self._append_transcript(user_entry)

        def _write_plan(task_list: List[Dict[str, Any]]) -> str:
            """Write plan to plans/{session_key}-{date}.json. Returns plan text for display."""
            try:
                ipc = self._get_ipc()
                r = ipc.plan_write(
                    session_key=self.session_key,
                    task_id=user_entry["id"],
                    task=user_message,
                    steps=task_list,
                    workspace_path=self.workspace_path,
                )
                return r.get("text", "") or ipc.plan_textify(task_list)
            except Exception as e:
                self._logger.warning("[Plan] plan_write failed: %s", e)
                return "\n".join(
                    f"{i+1}. {t.get('description', '')} [{t.get('tool_hint', '')}]"
                    for i, t in enumerate(task_list)
                )

        def _on_plan_generated(task_list: List[Dict[str, Any]]) -> None:
            """When task plan is generated: write to plan.json, show to user."""
            plan_text = _write_plan(task_list)
            if self.verbose and plan_text:
                print(f"\n📋 任务计划:\n{plan_text}\n")

        def _on_plan_updated(task_list: List[Dict[str, Any]]) -> None:
            """When step completes: overwrite plan.json with updated state."""
            _write_plan(task_list)

        # Create loop with custom tools
        custom_tools, tool_executor = self._build_custom_tools_and_executor()
        from ..config.env_config import get_planning_rules_path
        rules_path = get_planning_rules_path()
        loop = self.manager.create_enhanced_agentic_loop(
            client=self.client,
            model=self.model,
            system_prompt=system_content,
            max_iterations=self.max_iterations,
            max_tool_calls_per_task=self.max_tool_calls_per_task,
            custom_tools=custom_tools,
            custom_tool_executor=tool_executor,
            enable_task_planning=True,
            verbose=self.verbose,
            confirmation_callback=self.confirmation_callback,
            planning_rules_path=Path(rules_path) if rules_path else None,
        )

        response = loop.run(
            user_message,
            initial_messages=history,
            conversation_context=conv_ctx,
            timeout=None,
            on_plan_generated=_on_plan_generated,
            on_plan_updated=_on_plan_updated,
        )

        content = ""
        if response and response.choices:
            msg = response.choices[0].message
            content = msg.content or ""

        asst_entry = _message_to_transcript_entry("assistant", content, user_entry["id"])
        self._append_transcript(asst_entry)

        return content

    def close(self) -> None:
        """Close IPC client if any."""
        if self._ipc_client is not None:
            self._ipc_client.close()
            self._ipc_client = None
