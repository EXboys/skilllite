#!/usr/bin/env python3
"""
Example: Basic usage of SkillLite without LLM integration.

This example demonstrates how to use SkillLite directly
without connecting to any LLM API.
"""

import json
import os
import tempfile
from pathlib import Path

from skilllite import SkillManager


def create_example_skill(skills_dir: Path) -> Path:
    """Create an example skill for demonstration."""
    skill_dir = skills_dir / "calculator"
    skill_dir.mkdir(parents=True, exist_ok=True)
    
    # Create SKILL.md
    skill_md = """---
name: calculator
entry_point: main.py
language: python
description: A simple calculator that can add, subtract, multiply, and divide numbers.
input_schema:
  type: object
  properties:
    operation:
      type: string
      description: "The operation to perform: add, subtract, multiply, divide"
      enum: ["add", "subtract", "multiply", "divide"]
    a:
      type: number
      description: First operand
    b:
      type: number
      description: Second operand
  required:
    - operation
    - a
    - b
---

# Calculator Skill

A simple calculator that performs basic arithmetic operations.

## Usage

Provide an operation and two numbers to get the result.
"""
    (skill_dir / "SKILL.md").write_text(skill_md)
    
    # Create main.py
    main_py = '''#!/usr/bin/env python3
import json
import sys

def main():
    # Read input from stdin
    input_data = json.loads(sys.stdin.read())
    
    operation = input_data.get("operation", "add")
    a = float(input_data.get("a", 0))
    b = float(input_data.get("b", 0))
    
    if operation == "add":
        result = a + b
    elif operation == "subtract":
        result = a - b
    elif operation == "multiply":
        result = a * b
    elif operation == "divide":
        if b == 0:
            print(json.dumps({"error": "Division by zero"}))
            return
        result = a / b
    else:
        print(json.dumps({"error": f"Unknown operation: {operation}"}))
        return
    
    output = {
        "operation": operation,
        "a": a,
        "b": b,
        "result": result
    }
    
    print(json.dumps(output))

if __name__ == "__main__":
    main()
'''
    (skill_dir / "main.py").write_text(main_py)
    
    return skill_dir


def main():
    # Create a temporary directory with an example skill
    with tempfile.TemporaryDirectory() as temp_dir:
        skills_dir = Path(temp_dir) / "skills"
        skills_dir.mkdir()
        
        print("Creating example calculator skill...")
        create_example_skill(skills_dir)
        
        # Initialize the skill manager
        print(f"\nInitializing SkillManager with: {skills_dir}")
        manager = SkillManager(skills_dir=str(skills_dir))
        
        # List available skills
        print("\n=== Available Skills ===")
        for skill in manager.list_skills():
            print(f"Name: {skill.name}")
            print(f"Description: {skill.description}")
            print(f"Language: {skill.language}")
            print(f"Path: {skill.path}")
            print()
        
        # Get tool definitions
        print("=== Tool Definitions (Claude format) ===")
        tools = manager.get_tools_for_claude()
        for tool in tools:
            print(json.dumps(tool, indent=2))
        print()
        
        print("=== Tool Definitions (OpenAI format) ===")
        tools = manager.get_tools_for_openai()
        for tool in tools:
            print(json.dumps(tool, indent=2))
        print()
        
        # Execute the skill directly
        print("=== Direct Skill Execution ===")
        
        # Test addition
        print("\nTest 1: 5 + 3")
        result = manager.execute("calculator", {
            "operation": "add",
            "a": 5,
            "b": 3
        })
        if result.success:
            print(f"Result: {result.output}")
        else:
            print(f"Error: {result.error}")
        
        # Test multiplication
        print("\nTest 2: 7 * 6")
        result = manager.execute("calculator", {
            "operation": "multiply",
            "a": 7,
            "b": 6
        })
        if result.success:
            print(f"Result: {result.output}")
        else:
            print(f"Error: {result.error}")
        
        # Test division
        print("\nTest 3: 100 / 4")
        result = manager.execute("calculator", {
            "operation": "divide",
            "a": 100,
            "b": 4
        })
        if result.success:
            print(f"Result: {result.output}")
        else:
            print(f"Error: {result.error}")
        
        # Test division by zero
        print("\nTest 4: 10 / 0 (should error)")
        result = manager.execute("calculator", {
            "operation": "divide",
            "a": 10,
            "b": 0
        })
        if result.success:
            print(f"Result: {result.output}")
        else:
            print(f"Error: {result.error}")


if __name__ == "__main__":
    main()
