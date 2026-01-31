#!/usr/bin/env python3
"""
Interactive MCP Client Test Script

This script simulates a real LLM client interacting with the SkillLite MCP server,
demonstrating the two-phase security confirmation flow with actual user interaction.

Usage:
    python test_mcp_interactive.py

The script will:
1. Connect to the MCP server
2. Let you input code to execute
3. Show security scan results
4. Ask for your confirmation before executing risky code

Environment Variables (can be set in .env file):
    SKILLBOX_SANDBOX_LEVEL: Default sandbox level (1/2/3, default: 3)
"""

import asyncio
import json
import sys
import os
import re
from pathlib import Path

# Add skilllite-sdk to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'skilllite-sdk'))


def load_dotenv():
    """Load environment variables from .env file."""
    script_dir = Path(__file__).parent
    env_file = script_dir / ".env"
    
    if env_file.exists():
        with open(env_file, "r") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    key, _, value = line.partition("=")
                    key = key.strip()
                    value = value.strip()
                    if value and value[0] in ('"', "'") and value[-1] == value[0]:
                        value = value[1:-1]
                    if key and key not in os.environ:
                        os.environ[key] = value


# Load .env file
load_dotenv()


def print_header(title: str):
    print("\n" + "=" * 60)
    print(f"  {title}")
    print("=" * 60)


def print_section(title: str):
    print(f"\n{'─' * 40}")
    print(f"  {title}")
    print("─" * 40)


def colorize(text: str, color: str) -> str:
    """Add ANSI color to text."""
    colors = {
        "red": "\033[91m",
        "green": "\033[92m",
        "yellow": "\033[93m",
        "blue": "\033[94m",
        "magenta": "\033[95m",
        "cyan": "\033[96m",
        "reset": "\033[0m",
        "bold": "\033[1m",
    }
    return f"{colors.get(color, '')}{text}{colors['reset']}"


class InteractiveMCPClient:
    """Interactive MCP client that simulates real LLM behavior."""
    
    def __init__(self, session):
        self.session = session
    
    async def scan_code(self, language: str, code: str) -> dict:
        """Scan code for security issues."""
        result = await self.session.call_tool(
            "scan_code",
            arguments={"language": language, "code": code}
        )
        
        text = result.content[0].text if result.content else ""
        
        # Parse scan_id from response
        scan_id = None
        match = re.search(r'"scan_id":\s*"([^"]+)"', text)
        if match:
            scan_id = match.group(1)
        
        # Parse severity counts
        high_count = 0
        match = re.search(r'"high_severity_count":\s*(\d+)', text)
        if match:
            high_count = int(match.group(1))
        
        return {
            "text": text,
            "scan_id": scan_id,
            "high_severity_count": high_count,
            "requires_confirmation": high_count > 0,
        }
    
    async def execute_code(
        self,
        language: str,
        code: str,
        confirmed: bool = False,
        scan_id: str = None,
        sandbox_level: int = 3,
    ) -> dict:
        """Execute code in sandbox."""
        args = {
            "language": language,
            "code": code,
            "sandbox_level": sandbox_level,
        }
        if confirmed:
            args["confirmed"] = True
        if scan_id:
            args["scan_id"] = scan_id
        
        result = await self.session.call_tool("execute_code", arguments=args)
        
        text = result.content[0].text if result.content else ""
        is_error = getattr(result, 'isError', False)
        
        return {
            "text": text,
            "is_error": is_error,
            "requires_confirmation": "Security Review Required" in text,
        }


async def interactive_session():
    """Run an interactive MCP session."""
    print_header("🔐 SkillLite MCP Interactive Test")
    print("""
This demo simulates how an LLM client (like Claude Desktop) would
interact with the SkillLite MCP server for secure code execution.

The two-phase security model works as follows:
  1. scan_code  → Analyze code for security risks
  2. User reviews the security report
  3. execute_code (with confirmation) → Run the code if approved
""")
    
    try:
        from mcp import ClientSession, StdioServerParameters
        from mcp.client.stdio import stdio_client
    except ImportError:
        print(colorize("❌ MCP library not installed.", "red"))
        print("   Install with: pip install mcp")
        return False
    
    server_params = StdioServerParameters(
        command="skilllite",
        args=["mcp"],
        env=None
    )
    
    print(colorize("🔌 Connecting to MCP server...", "cyan"))
    
    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                await session.initialize()
                print(colorize("✅ Connected to SkillLite MCP server\n", "green"))
                
                client = InteractiveMCPClient(session)
                
                # Read default sandbox level from environment
                default_level_str = os.getenv("SKILLBOX_SANDBOX_LEVEL", "3")
                try:
                    default_sandbox_level = int(default_level_str)
                    if default_sandbox_level not in [1, 2, 3]:
                        default_sandbox_level = 3
                except ValueError:
                    default_sandbox_level = 3
                
                level_name = {1: "No Sandbox", 2: "Sandbox Only", 3: "Sandbox + Scan"}
                print(colorize(f"📋 Sandbox Level: {default_sandbox_level} ({level_name[default_sandbox_level]})", "cyan"))
                print(colorize("   (Set SKILLBOX_SANDBOX_LEVEL env to change)\n", "cyan"))
                
                # Main interaction loop
                while True:
                    print_section("Enter Code to Execute")
                    print("Commands: 'quit' to exit, 'demo' for demo with current level")
                    print()
                    
                    # Get language
                    language = input(colorize("Language [python/javascript/bash]: ", "cyan")).strip().lower()
                    if language == "quit":
                        break
                    if language == "demo":
                        await run_demo(client, sandbox_level=default_sandbox_level)
                        continue
                    if language not in ["python", "javascript", "bash"]:
                        language = "python"
                        print(f"  Using default: {language}")
                    
                    # Get code
                    print(colorize("\nEnter code (end with an empty line):", "cyan"))
                    code_lines = []
                    while True:
                        line = input()
                        if line == "":
                            break
                        code_lines.append(line)
                    
                    if not code_lines:
                        print(colorize("  No code entered, skipping...", "yellow"))
                        continue
                    
                    code = "\n".join(code_lines)
                    
                    # Execute with sandbox level from environment
                    await execute_with_confirmation(client, language, code, sandbox_level=default_sandbox_level)
                
                print(colorize("\n👋 Goodbye!", "green"))
                return True
                
    except FileNotFoundError:
        print(colorize("❌ skilllite command not found", "red"))
        print("   Install with: pip install -e ./skilllite-sdk")
        return False
    except Exception as e:
        print(colorize(f"❌ Error: {e}", "red"))
        import traceback
        traceback.print_exc()
        return False


async def run_demo(client: InteractiveMCPClient, sandbox_level: int = 3):
    """Run a demo with risky code."""
    level_name = "Level 3 (Sandbox + Scan)" if sandbox_level == 3 else "Level 2 (Sandbox Only)"
    print_section(f"🎬 Demo: Risky Code Execution ({level_name})")
    
    demo_code = '''import os
import subprocess

# This code accesses environment variables and could run commands
api_key = os.environ.get("API_KEY", "demo-key")
print(f"API Key: {api_key}")

# Simulated command (safe for demo)
result = "Command simulation: ls -la"
print(result)
'''
    
    print(colorize("Demo code:", "cyan"))
    print("─" * 40)
    for i, line in enumerate(demo_code.split("\n"), 1):
        print(f"  {i:2d} │ {line}")
    print("─" * 40)
    
    await execute_with_confirmation(client, "python", demo_code, sandbox_level=sandbox_level)


async def execute_with_confirmation(client: InteractiveMCPClient, language: str, code: str, sandbox_level: int = 3):
    """Execute code with the two-phase confirmation flow.
    
    Args:
        client: MCP client instance
        language: Programming language
        code: Code to execute
        sandbox_level: 1=no sandbox, 2=sandbox only (no scan), 3=sandbox+scan (default)
    """
    
    # Level 2: Skip scanning, execute directly in sandbox
    if sandbox_level <= 2:
        print_section("⚡ Direct Execution (Level 2: Sandbox Only)")
        print(colorize("  Skipping security scan, executing in sandbox...\n", "cyan"))
        exec_result = await client.execute_code(language, code, sandbox_level=sandbox_level)
        print(exec_result["text"])
        return
    
    # Level 3: Full security scan + confirmation flow
    # Phase 1: Security Scan
    print_section("📋 Phase 1: Security Scan")
    print(colorize("  Scanning code for security issues...\n", "cyan"))
    
    scan_result = await client.scan_code(language, code)
    
    # Display scan report (extract the readable part)
    report_lines = scan_result["text"].split("---")[0].strip()
    print(report_lines)
    
    if not scan_result["requires_confirmation"]:
        print(colorize("\n✅ No high-severity issues found. Proceeding to execution...", "green"))
        
        # Execute directly
        print_section("⚡ Phase 2: Execution")
        exec_result = await client.execute_code(language, code, sandbox_level=sandbox_level)
        print(exec_result["text"])
        return
    
    # Phase 2: User Confirmation
    print_section("🔐 Phase 2: User Confirmation Required")
    print(colorize(f"""
┌─────────────────────────────────────────────────────────────┐
│  ⚠️  Security Review Required                                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  The code contains {scan_result['high_severity_count']} high-severity security issue(s).       │
│                                                             │
│  This is a security prompt, not an error.                   │
│  If you trust this code, you can proceed safely.            │
│                                                             │
│  scan_id: {scan_result['scan_id'][:16] if scan_result['scan_id'] else 'N/A':16}                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
""", "yellow"))
    
    # Ask for user confirmation
    while True:
        response = input(colorize("  👉 Do you want to execute this code? [y/N]: ", "bold")).strip().lower()
        
        if response in ["y", "yes"]:
            print(colorize("\n  ✅ User approved - proceeding with execution...\n", "green"))
            
            # Execute with confirmation
            print_section("⚡ Phase 3: Confirmed Execution")
            exec_result = await client.execute_code(
                language,
                code,
                confirmed=True,
                scan_id=scan_result["scan_id"],
                sandbox_level=3,
            )
            print(exec_result["text"])
            break
            
        elif response in ["n", "no", ""]:
            print(colorize("\n  ⏹️  Execution cancelled by user.", "yellow"))
            break
            
        else:
            print(colorize("  Please enter 'y' to confirm or 'n' to cancel.", "yellow"))


async def main():
    result = await interactive_session()
    sys.exit(0 if result else 1)


if __name__ == "__main__":
    asyncio.run(main())
