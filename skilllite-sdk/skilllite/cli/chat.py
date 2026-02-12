"""
Chat command for skilllite CLI.

Provides the ``skilllite chat`` command for interactive multi-turn
conversation with persistent transcript and memory.

Requires skillbox built with --features chat.

Prerequisites:
  cd skillbox && cargo build --release --features chat

Usage:
  skilllite chat
  skilllite chat --workspace ~/.skilllite/chat
  skilllite chat --skills-dir ./.skills
"""

import argparse
import os
import sys
import time
from pathlib import Path

from ..quick import load_env


def cmd_chat(args: argparse.Namespace) -> int:
    """Run interactive chat session."""
    load_env()

    base_url = os.environ.get("BASE_URL")
    api_key = os.environ.get("API_KEY")
    model = os.environ.get("MODEL", "deepseek-chat")
    skills_dir = getattr(args, "skills_dir", None) or ".skills"
    workspace_path = getattr(args, "workspace", None) or str(Path.home() / ".skilllite" / "chat")
    session_key = getattr(args, "session", "main") or "main"

    if not api_key:
        print("Error: API_KEY not set. Set in .env or environment.")
        sys.exit(1)

    verbose = not getattr(args, "quiet", False)

    def _interactive_confirmation(report: str, scan_id: str) -> bool:
        """Prompt user for skill execution confirmation (sandbox_level=3)."""
        print(f"\n{report}")
        print("\n" + "=" * 60)
        while True:
            try:
                response = input("⚠️  Allow execution? (y/n): ").strip().lower()
            except (EOFError, KeyboardInterrupt):
                print("\nCancelled.")
                return False
            if response in ("y", "yes"):
                return True
            if response in ("n", "no"):
                return False
            print("Please enter 'y' or 'n'")

    try:
        from openai import OpenAI
        from ..core import SkillManager
        from ..core.chat_session import ChatSession

        client = OpenAI(base_url=base_url, api_key=api_key)
        manager = SkillManager(skills_dir=skills_dir)

        session = ChatSession(
            manager=manager,
            client=client,
            model=model,
            session_key=session_key,
            workspace_path=workspace_path,
            system_prompt="You are a helpful assistant with access to memory and file tools. "
                         "Use memory_search to recall past context, memory_write to store important information.",
            enable_builtin_tools=True,
            enable_memory_tools=True,
            verbose=verbose,
            confirmation_callback=_interactive_confirmation,
        )

        print("skilllite chat (session: %s)" % session_key)
        print("Ctrl+C or /exit to quit, /clear to clear history\n")

        while True:
            try:
                user_input = input("You: ").strip()
            except (EOFError, KeyboardInterrupt):
                print("\nBye.")
                break

            if not user_input:
                continue
            if user_input.lower() in ("/exit", "/quit", "/q"):
                print("Bye.")
                break
            if user_input.lower() == "/clear":
                session_key = f"main_{int(time.time())}"
                session.session_key = session_key
                session._ensure_session()
                print("Session cleared (new session).")
                continue

            try:
                reply = session.run_turn(user_input)
                print(f"\nAssistant: {reply}\n")
            except Exception as e:
                if "Method not found" in str(e):
                    print("\nError: Chat feature not enabled in skillbox.")
                    print("  Build with: cd skillbox && cargo build --release --features chat")
                    sys.exit(1)
                print(f"\nError: {e}\n")

        session.close()
        return 0

    except ImportError as e:
        print(f"Error: {e}")
        sys.exit(1)
