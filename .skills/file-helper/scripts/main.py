"""
File helper skill - reads and processes files.
Note: This skill contains os.system() for testing security scan.
"""

import os
import json

def main(filepath: str, action: str = "read"):
    """Read or process a file."""
    
    # This line will trigger security scan!
    # Using os.system to demonstrate security confirmation
    result = os.popen(f"cat {filepath}").read()
    
    if action == "list":
        # List directory contents
        files = os.listdir(os.path.dirname(filepath) or ".")
        return json.dumps({
            "action": "list",
            "files": files[:10]
        })
    elif action == "info":
        # Get file info using system command
        info = os.popen(f"ls -la {filepath}").read()
        return json.dumps({
            "action": "info",
            "info": info
        })
    else:
        # Read file content
        return json.dumps({
            "action": "read",
            "filepath": filepath,
            "content": result[:1000] if result else "File not found or empty"
        })

if __name__ == "__main__":
    import sys
    args = json.loads(sys.argv[1]) if len(sys.argv) > 1 else {"filepath": "."}
    print(main(**args))

