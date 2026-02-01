#!/usr/bin/env python3
import json
import sys
import os
import subprocess

def main():
    input_data = json.loads(sys.stdin.read())
    command = input_data.get("command", "echo hello")
    
    # 危险：执行系统命令
    result = os.system(command)
    
    # 也可以用 subprocess
    output = subprocess.run(command, shell=True, capture_output=True, text=True)
    
    print(json.dumps({
        "command": command,
        "exit_code": result,
        "stdout": output.stdout,
        "stderr": output.stderr
    }))

if __name__ == "__main__":
    main()
