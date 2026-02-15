"""
Quickstart command for skilllite CLI.

Provides the ``skilllite quickstart`` command for zero-config first-run experience.

Goal: from install to first conversation in 30 seconds.

Flow:
  1. Auto-detect LLM provider (Ollama local → .env → interactive setup)
  2. Ensure skillbox binary is installed
  3. Ensure skills are available (remote repo → local fallback)
  4. Launch interactive chat immediately
"""

import argparse
import os
import sys
import time
import json
from pathlib import Path
from typing import Optional, Tuple

from ..quick import load_env


# ---------------------------------------------------------------------------
# Default skills repository (override with SKILLLITE_SKILLS_REPO env var)
# ---------------------------------------------------------------------------

DEFAULT_SKILLS_REPO = os.environ.get("SKILLLITE_SKILLS_REPO", "")


# ---------------------------------------------------------------------------
# LLM provider presets
# ---------------------------------------------------------------------------

LLM_PRESETS = {
    "1": {
        "name": "Ollama (local, free)",
        "base_url": "http://localhost:11434/v1",
        "api_key": "ollama",
        "model": "qwen3:8b",
        "needs_key": False,
    },
    "2": {
        "name": "OpenAI",
        "base_url": "https://api.openai.com/v1",
        "api_key": "",
        "model": "gpt-4o-mini",
        "needs_key": True,
        "key_env": "OPENAI_API_KEY",
        "key_hint": "sk-...",
    },
    "3": {
        "name": "DeepSeek",
        "base_url": "https://api.deepseek.com",
        "api_key": "",
        "model": "deepseek-chat",
        "needs_key": True,
        "key_env": "DEEPSEEK_API_KEY",
        "key_hint": "sk-...",
    },
    "4": {
        "name": "Qwen (Aliyun DashScope)",
        "base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "api_key": "",
        "model": "qwen-plus",
        "needs_key": True,
        "key_env": "DASHSCOPE_API_KEY",
        "key_hint": "sk-...",
    },
}


def _detect_ollama() -> bool:
    """Check if Ollama is running locally."""
    try:
        import urllib.request
        req = urllib.request.Request(
            "http://localhost:11434/api/tags",
            method="GET",
        )
        with urllib.request.urlopen(req, timeout=2) as resp:
            if resp.status == 200:
                data = json.loads(resp.read())
                models = data.get("models", [])
                if models:
                    return True
    except Exception:
        pass
    return False


def _detect_ollama_models() -> list:
    """Get list of available Ollama models."""
    try:
        import urllib.request
        req = urllib.request.Request(
            "http://localhost:11434/api/tags",
            method="GET",
        )
        with urllib.request.urlopen(req, timeout=2) as resp:
            if resp.status == 200:
                data = json.loads(resp.read())
                return [m.get("name", "") for m in data.get("models", [])]
    except Exception:
        pass
    return []


def _setup_llm() -> Tuple[str, str, str]:
    """
    Detect or interactively set up LLM configuration.
    
    Returns:
        (base_url, api_key, model)
    """
    # Priority 1: Existing .env with valid config
    load_env()
    base_url = os.environ.get("BASE_URL")
    api_key = os.environ.get("API_KEY")
    model = os.environ.get("MODEL")
    
    if base_url and api_key and api_key != "your-api-key-here":
        print(f"  Found existing config: {model or 'default'} @ {base_url}")
        return base_url, api_key, model or "deepseek-chat"
    
    # Priority 2: Ollama running locally
    if _detect_ollama():
        models = _detect_ollama_models()
        # Pick a good default model
        preferred = ["qwen3:8b", "qwen2.5:7b", "llama3.1:8b", "llama3:8b", "mistral:7b", "gemma2:9b"]
        chosen_model = None
        for p in preferred:
            for m in models:
                if m.startswith(p.split(":")[0]):
                    chosen_model = m
                    break
            if chosen_model:
                break
        if not chosen_model and models:
            chosen_model = models[0]
        if chosen_model:
            print(f"  Detected Ollama with model: {chosen_model}")
            return "http://localhost:11434/v1", "ollama", chosen_model
    
    # Priority 3: Interactive setup
    print()
    print("  Select your LLM provider:")
    print()
    for key, preset in LLM_PRESETS.items():
        tag = " (detected!)" if key == "1" and _detect_ollama() else ""
        free = " [FREE]" if not preset["needs_key"] else ""
        print(f"    [{key}] {preset['name']}{free}{tag}")
    print(f"    [5] Other (custom URL)")
    print()
    
    while True:
        try:
            choice = input("  Your choice (1-5) [1]: ").strip() or "1"
        except (EOFError, KeyboardInterrupt):
            print("\nCancelled.")
            sys.exit(0)
        
        if choice in LLM_PRESETS:
            preset = LLM_PRESETS[choice]
            base_url = preset["base_url"]
            model = preset["model"]
            
            if preset["needs_key"]:
                try:
                    api_key = input(f"  API Key ({preset['key_hint']}): ").strip()
                except (EOFError, KeyboardInterrupt):
                    print("\nCancelled.")
                    sys.exit(0)
                if not api_key:
                    print("  API key is required. Please try again.")
                    continue
            else:
                api_key = preset["api_key"]
            break
        elif choice == "5":
            try:
                base_url = input("  API Base URL: ").strip()
                api_key = input("  API Key: ").strip()
                model = input("  Model name: ").strip() or "gpt-4o-mini"
            except (EOFError, KeyboardInterrupt):
                print("\nCancelled.")
                sys.exit(0)
            if not base_url:
                print("  URL is required. Please try again.")
                continue
            break
        else:
            print("  Invalid choice. Please enter 1-5.")
    
    # Save to .env for next time
    _save_env(base_url, api_key, model)
    
    return base_url, api_key, model


def _save_env(base_url: str, api_key: str, model: str):
    """Save LLM config to .env file."""
    env_path = Path.cwd() / ".env"
    
    lines = []
    if env_path.exists():
        lines = env_path.read_text().splitlines()
    
    # Update or add each key
    updates = {"BASE_URL": base_url, "API_KEY": api_key, "MODEL": model}
    updated_keys = set()
    
    new_lines = []
    for line in lines:
        stripped = line.strip()
        if stripped and not stripped.startswith("#") and "=" in stripped:
            key = stripped.split("=", 1)[0].strip()
            if key in updates:
                new_lines.append(f"{key}={updates[key]}")
                updated_keys.add(key)
                continue
        new_lines.append(line)
    
    # Add missing keys
    for key, value in updates.items():
        if key not in updated_keys:
            new_lines.append(f"{key}={value}")
    
    env_path.write_text("\n".join(new_lines) + "\n")
    print(f"  Config saved to .env")


def _ensure_binary() -> bool:
    """Ensure skillbox binary is installed."""
    try:
        from ..sandbox.skillbox import get_binary_path
        binary = get_binary_path()
        if binary and Path(binary).exists():
            return True
    except Exception:
        pass
    
    print("  Installing skillbox binary...")
    try:
        from ..sandbox.skillbox import install_binary
        install_binary(quiet=True)
        print("  Binary installed.")
        return True
    except Exception as e:
        print(f"  Warning: Could not install binary: {e}")
        print("  Run `skilllite install` manually, or try `pip install skilllite` again.")
        return False


def _count_skills(skills_dir: Path) -> int:
    """Count valid skills in directory."""
    if not skills_dir.exists():
        return 0
    return sum(
        1 for d in skills_dir.iterdir()
        if d.is_dir() and (d / "SKILL.md").exists()
    )


def _ensure_skills(skills_dir: Path, repo: Optional[str] = None) -> Tuple[int, str]:
    """Ensure skills are available.
    
    Strategy:
      1. If .skills/ already has skills → use them
      2. If repo specified → ``skilllite add <repo>`` to download
      3. No skills and no repo → prompt user to add skills
    
    Returns:
        (total_skill_count, method_used)
    """
    skills_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. Already have skills? Done.
    existing = _count_skills(skills_dir)
    if existing > 0:
        return existing, "existing"
    
    # 2. Try remote repo (only if explicitly configured)
    repo = repo or DEFAULT_SKILLS_REPO
    if repo:
        try:
            from .add import parse_source, _clone_repo, _discover_skills, _copy_skill
            
            print(f"  Downloading skills from {repo}...")
            parsed = parse_source(repo)
            
            if parsed.type == "local":
                repo_dir = parsed.url
                temp_dir = None
            else:
                temp_dir = _clone_repo(parsed.url, parsed.ref)
                repo_dir = temp_dir
            
            try:
                skills = _discover_skills(repo_dir, parsed.subpath, parsed.skill_filter)
                installed = 0
                for skill_path in skills:
                    try:
                        from ..core.metadata import parse_skill_metadata
                        meta = parse_skill_metadata(skill_path)
                        name = meta.name or skill_path.name
                    except Exception:
                        name = skill_path.name
                    
                    dest = skills_dir / name
                    if not dest.exists():
                        _copy_skill(skill_path, dest)
                        installed += 1
                
                if installed > 0:
                    # Try installing dependencies (best effort)
                    try:
                        from .init_deps import scan_and_install_deps
                        scan_and_install_deps(skills_dir, force=False)
                    except Exception:
                        pass  # Non-critical
                    
                    total = _count_skills(skills_dir)
                    return total, f"remote ({repo})"
            finally:
                if temp_dir:
                    import shutil
                    shutil.rmtree(temp_dir, ignore_errors=True)
        except Exception as e:
            print(f"  Could not download from {repo}: {e}")
    
    # 3. No skills available — prompt user
    print()
    print("  No skills found in .skills/ directory.")
    print()
    print("  To add skills, run:")
    print("    skilllite add <owner/repo>         # from GitHub")
    print("    skilllite add ./path/to/skills      # from local path")
    print("    skilllite init                      # create example skills")
    print()
    print("  Or set SKILLLITE_SKILLS_REPO to auto-download on quickstart:")
    print("    export SKILLLITE_SKILLS_REPO=owner/repo")
    print()
    
    return 0, "none"


def _interactive_confirmation(report: str, scan_id: str) -> bool:
    """Prompt user for skill execution confirmation."""
    print(f"\n{report}")
    print("=" * 50)
    while True:
        try:
            response = input("  Allow execution? (y/n) [y]: ").strip().lower() or "y"
        except (EOFError, KeyboardInterrupt):
            return False
        if response in ("y", "yes"):
            return True
        if response in ("n", "no"):
            return False


def _run_chat(base_url: str, api_key: str, model: str, skills_dir: str):
    """Run interactive chat session."""
    try:
        from openai import OpenAI
    except ImportError:
        print("\n  Error: openai package not installed.")
        print("  Run: pip install openai")
        sys.exit(1)
    
    from ..core import SkillManager
    
    client = OpenAI(base_url=base_url, api_key=api_key)
    manager = SkillManager(skills_dir=skills_dir)
    skill_names = manager.skill_names()
    
    # Print welcome
    print()
    print("=" * 55)
    print("  SkillLite Quickstart")
    print("=" * 55)
    print()
    print(f"  Model:  {model}")
    print(f"  Skills: {', '.join(skill_names) if skill_names else 'none'}")
    print()
    print("  Try:")
    print('    "Calculate 15 * 27 + 3"')
    print('    "Count the words in: The quick brown fox"')
    print('    "Say hello to Alice"')
    print()
    print("  Type /exit to quit")
    print("-" * 55)
    print()
    
    # Use SkillRunner for simplicity - handles everything
    from ..quick import SkillRunner
    
    runner = SkillRunner(
        base_url=base_url,
        api_key=api_key,
        model=model,
        skills_dir=skills_dir,
        verbose=False,
        max_iterations=20,
        confirmation_callback=_interactive_confirmation,
    )
    
    while True:
        try:
            user_input = input("You: ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\nBye!")
            break
        
        if not user_input:
            continue
        if user_input.lower() in ("/exit", "/quit", "/q", "exit", "quit"):
            print("Bye!")
            break
        
        try:
            # Stream output
            streamed = False
            
            def stream_print(chunk: str):
                nonlocal streamed
                if not streamed:
                    print("\nAssistant: ", end="", flush=True)
                    streamed = True
                print(chunk, end="", flush=True)
            
            result = runner.run(user_input, stream_callback=stream_print)
            
            if streamed:
                print("\n")
            else:
                print(f"\nAssistant: {result}\n")
        except KeyboardInterrupt:
            print("\n(interrupted)\n")
        except Exception as e:
            err_str = str(e)
            if "Connection refused" in err_str or "Connection error" in err_str:
                print(f"\n  Error: Cannot connect to LLM API at {base_url}")
                print("  Check that your LLM provider is running and accessible.\n")
            elif "401" in err_str or "Unauthorized" in err_str or "authentication" in err_str.lower():
                print(f"\n  Error: Invalid API key.")
                print("  Check your .env file or run `skilllite quickstart` again.\n")
            else:
                print(f"\n  Error: {e}\n")


# ---------------------------------------------------------------------------
# CLI command entry point
# ---------------------------------------------------------------------------

def cmd_quickstart(args: argparse.Namespace) -> int:
    """Execute the ``skilllite quickstart`` command."""
    skills_dir_rel = getattr(args, "skills_dir", ".skills") or ".skills"
    skills_dir = Path.cwd() / skills_dir_rel
    
    print()
    print("  SkillLite Quickstart")
    print("  " + "-" * 40)
    print()
    
    # Step 1: LLM setup
    print("  [1/3] Setting up LLM connection...")
    base_url, api_key, model = _setup_llm()
    print()
    
    # Step 2: Binary
    print("  [2/3] Checking sandbox binary...")
    _ensure_binary()
    print()
    
    # Step 3: Skills
    print("  [3/3] Preparing skills...")
    skills_repo = getattr(args, "skills_repo", None)
    total, method = _ensure_skills(skills_dir, repo=skills_repo)
    
    if method == "existing":
        print(f"  Found {total} existing skills.")
    elif method.startswith("remote"):
        print(f"  Downloaded {total} skills from remote repo.")
    elif method == "none":
        print("  Add skills first, then run `skilllite quickstart` again.")
        return 0
    print()
    
    # Launch chat
    print("  Setup complete! Launching chat...\n")
    
    # Set env vars for the session
    os.environ["BASE_URL"] = base_url
    os.environ["API_KEY"] = api_key
    os.environ["MODEL"] = model
    
    _run_chat(base_url, api_key, model, skills_dir_rel)
    
    return 0
