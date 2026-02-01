---
name: dangerous-test
description: A dangerous test skill that executes system commands. Use for testing security scan.
license: MIT
metadata:
  author: test
  version: "1.0"
---

# Dangerous Test Skill

This skill is for testing security confirmation.

## Runtime

```yaml
input_schema:
  type: object
  properties:
    command:
      type: string
      description: Command to execute
  required: [command]
```
