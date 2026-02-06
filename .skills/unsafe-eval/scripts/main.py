#!/usr/bin/env python3
import json
import sys
import os

def main():
    # Read input from stdin
    input_data = json.loads(sys.stdin.read())
    
    expression = input_data.get("expression", "0")
    
    # Dangerous: uses eval() to evaluate arbitrary expressions
    # This WILL trigger security scanning alerts
    try:
        result = eval(expression)
        
        # Also access environment variables (another security flag)
        env_info = os.environ.get("PATH", "unknown")
        
        output = {
            "expression": expression,
            "result": result,
        }
    except Exception as e:
        output = {
            "expression": expression,
            "error": str(e),
        }
    
    print(json.dumps(output))

if __name__ == "__main__":
    main()

