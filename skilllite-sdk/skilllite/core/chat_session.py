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
    """Get SkillboxIPCClient for chat RPC. Requires skillbox built with --features chat."""
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
        max_iterations: int = 10,
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

    def _memory_search(self, query: str, limit: int = 10) -> List[Dict[str, Any]]:
        """Search memory (BM25)."""
        ipc = self._get_ipc()
        return ipc.memory_search(
            query=query,
            limit=limit,
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
        hits = self._memory_search(query=user_message, limit=limit)
        if not hits:
            return ""
        parts = ["## Relevant Memory\n"]
        for h in hits:
            c = h.get("content", "")
            if c:
                parts.append(f"- {c[:300]}{'...' if len(c) > 300 else ''}")
        return "\n".join(parts) if len(parts) > 1 else ""

    def _build_custom_tools_and_executor(self):
        """Build custom tools (builtin + memory) and combined executor."""
        from .chat_tools import get_memory_tools, create_memory_tool_executor

        custom_tools: List[Dict[str, Any]] = []
        builtin_executor = None
        memory_executor = None

        builtin_names = {"read_file", "write_file", "list_directory", "file_exists", "run_command"}
        memory_names = {"memory_search", "memory_write", "memory_list"}

        if self.enable_builtin_tools:
            from ..builtin_tools import get_builtin_file_tools, create_builtin_tool_executor
            workspace_root = Path(self.workspace_path).resolve()
            custom_tools.extend(get_builtin_file_tools())
            builtin_executor = create_builtin_tool_executor(
                run_command_confirmation=self.confirmation_callback,
                workspace_root=workspace_root,
            )

        if self.enable_memory_tools:
            custom_tools.extend(get_memory_tools())
            memory_executor = create_memory_tool_executor(workspace_path=self.workspace_path)

        def combined_executor(tool_input: Dict[str, Any]) -> str:
            tool_name = tool_input.get("tool_name", "")
            if tool_name in builtin_names and builtin_executor:
                return builtin_executor(tool_input)
            if tool_name in memory_names and memory_executor:
                return memory_executor(tool_input)
            return f"Error: No executor for tool: {tool_name}"

        return custom_tools, combined_executor

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

        # Create loop with custom tools
        custom_tools, tool_executor = self._build_custom_tools_and_executor()
        loop = self.manager.create_enhanced_agentic_loop(
            client=self.client,
            model=self.model,
            system_prompt=system_content,
            max_iterations=self.max_iterations,
            custom_tools=custom_tools,
            custom_tool_executor=tool_executor,
            enable_task_planning=False,
            verbose=self.verbose,
            confirmation_callback=self.confirmation_callback,
        )

        # Run - we pass messages directly via a custom flow
        # The loop normally takes user_message and builds messages. We need to inject history.
        # AgenticLoop.run() builds messages from user_message. We need to override.
        # Option: pass user_message as the latest, but the loop prepends system and adds user.
        # The loop doesn't support pre-built messages. We need to either:
        # 1. Extend AgenticLoop to accept initial_messages
        # 2. Or run the loop with a concatenated "history as context" in the user message
        # 3. Or run a modified loop that accepts messages

        # Simpler: pass the full conversation as a single "user" message that includes history.
        # No - that would confuse the model. Better to extend the loop.

        # Check AgenticLoop.run - it calls _run_openai(user_message). The messages are built
        # from system_prompt + user_message. We need to inject history. The cleanest is to
        # add an optional initial_messages parameter to run().

        # For now: use a workaround - build a synthetic user message that includes history.
        # Format: "Previous conversation:\nUser: ...\nAssistant: ...\n\nCurrent: {user_message}"
        # This is suboptimal. Better to extend AgenticLoop.

        # Let me extend AgenticLoop.run to accept optional initial_messages.
        response = loop.run(
            user_message,
            initial_messages=history,
            timeout=None,
        )

        content = ""
        if response and response.choices:
            msg = response.choices[0].message
            content = msg.content or ""

        # Append to transcript
        last_id = entries[-1].get("id") if entries else None
        user_entry = _message_to_transcript_entry("user", user_message, last_id)
        self._append_transcript(user_entry)
        asst_entry = _message_to_transcript_entry("assistant", content, user_entry["id"])
        self._append_transcript(asst_entry)

        return content

    def close(self) -> None:
        """Close IPC client if any."""
        if self._ipc_client is not None:
            self._ipc_client.close()
            self._ipc_client = None
