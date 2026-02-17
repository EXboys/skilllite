---
name: example-python-skill
entry_point: script/main.py
language: python
description: A simple example skill that echoes input
version: "1.0.0"
network:
  enabled: false
  outbound: []
  block_private_ips: true
input_schema:
  type: object
  properties:
    message:
      type: string
      description: The message to echo
  required:
    - message
output_schema:
  type: object
  properties:
    result:
      type: string
    input:
      type: object
---

# Example Python Skill

This is a simple example skill that demonstrates the SkillBox protocol.

## Usage

```bash
skillbox run ./examples/python_skill '{"message": "Hello, World!"}'
```

## Input

- `message` (string, required): The message to echo back

## Output

- `result`: Status of the operation
- `input`: The original input parameters
