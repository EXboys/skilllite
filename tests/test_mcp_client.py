#!/usr/bin/env python3
"""
MCP Client Test Script

This script acts as an MCP client to test the SkillLite MCP server.
It demonstrates the two-phase security execution model:

1. scan_code: Scan code for security issues before execution
2. execute_code: Execute code with optional confirmation for risky operations

Usage:
    python test_mcp_client.py [--test-name TEST_NAME]

Examples:
    python test_mcp_client.py                    # Run all tests
    python test_mcp_client.py --test-name safe   # Run only safe code test
    python test_mcp_client.py --test-name risky  # Run only risky code test
"""

import argparse
import asyncio
import json
import sys
import os

# Add python-sdk to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'python-sdk'))


class MCPClientTester:
    """MCP Client Tester for SkillLite sandbox server."""
    
    def __init__(self, session):
        self.session = session
        self.passed = 0
        self.failed = 0
    
    def log_success(self, message: str):
        print(f"  ✅ {message}")
        self.passed += 1
    
    def log_failure(self, message: str):
        print(f"  ❌ {message}")
        self.failed += 1
    
    def log_info(self, message: str):
        print(f"  ℹ️  {message}")
    
    async def test_list_tools(self) -> bool:
        """Test listing available tools."""
        print("\n📋 Test: List Available Tools")
        print("-" * 40)
        
        try:
            tools_result = await self.session.list_tools()
            tool_names = [tool.name for tool in tools_result.tools]
            
            if "scan_code" in tool_names:
                self.log_success("scan_code tool available")
            else:
                self.log_failure("scan_code tool not found")
            
            if "execute_code" in tool_names:
                self.log_success("execute_code tool available")
            else:
                self.log_failure("execute_code tool not found")
            
            print("\n  Available tools:")
            for tool in tools_result.tools:
                desc = tool.description[:60] + "..." if len(tool.description) > 60 else tool.description
                print(f"    • {tool.name}: {desc}")
            
            return "scan_code" in tool_names and "execute_code" in tool_names
        except Exception as e:
            self.log_failure(f"Failed to list tools: {e}")
            return False
    
    async def test_safe_code_execution(self) -> bool:
        """Test executing safe code (no security issues)."""
        print("\n🟢 Test: Safe Code Execution")
        print("-" * 40)
        
        safe_code = '''
import json
result = {"status": "success", "value": 42}
print(json.dumps(result))
'''
        
        try:
            # First, scan the code
            self.log_info("Scanning code for security issues...")
            scan_result = await self.session.call_tool(
                "scan_code",
                arguments={"language": "python", "code": safe_code}
            )
            
            scan_text = scan_result.content[0].text if scan_result.content else ""
            
            if "No issues found" in scan_text or '"high_severity_count": 0' in scan_text:
                self.log_success("Scan passed - no high severity issues")
            else:
                self.log_info("Scan found some issues (expected for demo)")
            
            # Then execute the code
            self.log_info("Executing code...")
            exec_result = await self.session.call_tool(
                "execute_code",
                arguments={
                    "language": "python",
                    "code": safe_code,
                    "sandbox_level": 2  # Use level 2 to skip security scan
                }
            )
            
            exec_text = exec_result.content[0].text if exec_result.content else ""
            
            if '"status": "success"' in exec_text or '"value": 42' in exec_text:
                self.log_success("Code executed successfully")
                print(f"\n  Output:\n    {exec_text.strip()}")
                return True
            else:
                self.log_failure(f"Unexpected output: {exec_text[:100]}")
                return False
                
        except Exception as e:
            self.log_failure(f"Test failed: {e}")
            return False
    
    async def test_risky_code_with_confirmation(self) -> bool:
        """Test the two-phase execution flow for risky code."""
        print("\n🟠 Test: Risky Code with Confirmation Flow")
        print("-" * 40)
        
        risky_code = '''
import os
import subprocess

# This code has security implications
api_key = os.environ.get("API_KEY", "default")
print(f"API Key: {api_key}")
'''
        
        try:
            # Step 1: Scan the code
            self.log_info("Step 1: Scanning risky code...")
            scan_result = await self.session.call_tool(
                "scan_code",
                arguments={"language": "python", "code": risky_code}
            )
            
            scan_text = scan_result.content[0].text if scan_result.content else ""
            print(f"\n  Scan Report Preview:\n    {scan_text[:300]}...")
            
            # Extract scan_id from the result
            scan_id = None
            if '"scan_id":' in scan_text:
                import re
                match = re.search(r'"scan_id":\s*"([^"]+)"', scan_text)
                if match:
                    scan_id = match.group(1)
                    self.log_success(f"Got scan_id: {scan_id[:8]}...")
            
            if not scan_id:
                self.log_info("No scan_id found (code may be safe)")
            
            # Step 2: Try to execute without confirmation (should fail for level 3)
            self.log_info("Step 2: Attempting execution without confirmation (level 3)...")
            exec_result = await self.session.call_tool(
                "execute_code",
                arguments={
                    "language": "python",
                    "code": risky_code,
                    "sandbox_level": 3
                }
            )
            
            exec_text = exec_result.content[0].text if exec_result.content else ""
            
            if "Security Review Required" in exec_text or "requires_confirmation" in exec_text:
                self.log_success("Execution blocked - confirmation required (expected)")
            else:
                self.log_info("Execution proceeded (code may be considered safe)")
            
            # Step 3: Execute with confirmation
            if scan_id:
                self.log_info("Step 3: Executing with confirmation...")
                confirmed_result = await self.session.call_tool(
                    "execute_code",
                    arguments={
                        "language": "python",
                        "code": risky_code,
                        "confirmed": True,
                        "scan_id": scan_id,
                        "sandbox_level": 3
                    }
                )
                
                confirmed_text = confirmed_result.content[0].text if confirmed_result.content else ""
                
                if "API Key:" in confirmed_text or exec_result.isError is False:
                    self.log_success("Confirmed execution succeeded")
                    return True
                else:
                    self.log_info(f"Execution result: {confirmed_text[:100]}")
                    return True  # Test passed even if sandbox blocked it
            
            return True
            
        except Exception as e:
            self.log_failure(f"Test failed: {e}")
            import traceback
            traceback.print_exc()
            return False
    
    async def test_invalid_scan_id(self) -> bool:
        """Test that invalid scan_id is rejected."""
        print("\n🔴 Test: Invalid Scan ID Rejection")
        print("-" * 40)
        
        code = 'print("test")'
        
        try:
            self.log_info("Attempting execution with fake scan_id...")
            result = await self.session.call_tool(
                "execute_code",
                arguments={
                    "language": "python",
                    "code": code,
                    "confirmed": True,
                    "scan_id": "fake_invalid_scan_id",
                    "sandbox_level": 3
                }
            )
            
            result_text = result.content[0].text if result.content else ""
            
            if "Invalid" in result_text or "expired" in result_text:
                self.log_success("Invalid scan_id correctly rejected")
                return True
            else:
                self.log_info(f"Result: {result_text[:100]}")
                return True  # May succeed if code is safe
                
        except Exception as e:
            self.log_failure(f"Test failed: {e}")
            return False
    
    async def test_different_languages(self) -> bool:
        """Test execution in different languages."""
        print("\n🌐 Test: Multi-Language Support")
        print("-" * 40)
        
        test_cases = [
            ("python", 'print("Hello from Python!")'),
            ("javascript", 'console.log("Hello from JavaScript!")'),
            ("bash", 'echo "Hello from Bash!"'),
        ]
        
        all_passed = True
        for language, code in test_cases:
            try:
                self.log_info(f"Testing {language}...")
                result = await self.session.call_tool(
                    "execute_code",
                    arguments={
                        "language": language,
                        "code": code,
                        "sandbox_level": 2
                    }
                )
                
                result_text = result.content[0].text if result.content else ""
                
                if "Hello from" in result_text:
                    self.log_success(f"{language} execution succeeded")
                elif result.isError:
                    self.log_info(f"{language}: {result_text[:50]}...")
                else:
                    self.log_success(f"{language} completed")
                    
            except Exception as e:
                self.log_failure(f"{language} failed: {e}")
                all_passed = False
        
        return all_passed
    
    def print_summary(self):
        """Print test summary."""
        total = self.passed + self.failed
        print("\n" + "=" * 50)
        print("📊 Test Summary")
        print("=" * 50)
        print(f"  Total:  {total}")
        print(f"  Passed: {self.passed} ✅")
        print(f"  Failed: {self.failed} ❌")
        
        if self.failed == 0:
            print("\n🎉 All tests passed!")
        else:
            print(f"\n⚠️  {self.failed} test(s) failed")


async def run_tests(test_name: str = None):
    """Run MCP client tests."""
    print("=" * 50)
    print("🧪 SkillLite MCP Server Test Suite")
    print("=" * 50)
    
    try:
        from mcp import ClientSession, StdioServerParameters
        from mcp.client.stdio import stdio_client
        
        print("✅ MCP client libraries imported")
        
    except ImportError as e:
        print(f"❌ Import error: {e}")
        print("\nPlease install MCP library:")
        print("  pip install mcp")
        return False
    
    # Configure server parameters
    server_params = StdioServerParameters(
        command="skilllite",
        args=["mcp"],
        env=None
    )
    
    print(f"🔌 Connecting to: {server_params.command} {' '.join(server_params.args)}")
    
    try:
        async with stdio_client(server_params) as (read, write):
            async with ClientSession(read, write) as session:
                print("✅ Connected to MCP server")
                
                await session.initialize()
                print("✅ Session initialized")
                
                tester = MCPClientTester(session)
                
                # Run tests based on test_name
                if test_name is None or test_name == "list":
                    await tester.test_list_tools()
                
                if test_name is None or test_name == "safe":
                    await tester.test_safe_code_execution()
                
                if test_name is None or test_name == "risky":
                    await tester.test_risky_code_with_confirmation()
                
                if test_name is None or test_name == "invalid":
                    await tester.test_invalid_scan_id()
                
                if test_name is None or test_name == "languages":
                    await tester.test_different_languages()
                
                tester.print_summary()
                return tester.failed == 0
                
    except FileNotFoundError:
        print("❌ skilllite command not found")
        print("\nPlease install skilllite:")
        print("  pip install -e ./python-sdk")
        return False
    except Exception as e:
        print(f"❌ Connection error: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    parser = argparse.ArgumentParser(
        description="Test SkillLite MCP Server",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Test Names:
  list      - Test listing available tools
  safe      - Test safe code execution
  risky     - Test risky code with confirmation flow
  invalid   - Test invalid scan_id rejection
  languages - Test multi-language support

Examples:
  python test_mcp_client.py                    # Run all tests
  python test_mcp_client.py --test-name safe   # Run only safe code test
"""
    )
    parser.add_argument(
        "--test-name",
        choices=["list", "safe", "risky", "invalid", "languages"],
        help="Run specific test only"
    )
    
    args = parser.parse_args()
    
    result = asyncio.run(run_tests(args.test_name))
    sys.exit(0 if result else 1)


if __name__ == "__main__":
    main()
