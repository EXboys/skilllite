#!/usr/bin/env python3
"""
Example SkillBox - Echo Input

This skill reads JSON input from stdin and outputs JSON to stdout.
"""

import json
import sys


def main():
    try:
        # Read input from stdin
        input_data = json.load(sys.stdin)
        
        # Process the input (in this case, just echo it back)
        output = {
            "result": "ok",
            "input": input_data,
            "message": f"Received: {input_data.get('message', 'No message provided')}"
        }
        
        # Output result as JSON to stdout
        print(json.dumps(output))
        
    except json.JSONDecodeError as e:
        # Handle JSON parsing errors
        error_output = {
            "result": "error",
            "error": f"Invalid JSON input: {str(e)}"
        }
        print(json.dumps(error_output), file=sys.stdout)
        sys.exit(1)
        
    except Exception as e:
        # Handle other errors
        error_output = {
            "result": "error", 
            "error": str(e)
        }
        print(json.dumps(error_output), file=sys.stdout)
        sys.exit(1)


if __name__ == "__main__":
    main()
