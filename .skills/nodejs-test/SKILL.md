---
name: node-test
version: 1.0.0
description: A simple Node.js skill for testing
language: node
entry_point: scripts/main.js
network:
  enabled: false
input_schema:
  type: object
  properties:
    text:
      type: string
      description: Text to process
  required:
    - text
output_schema:
  type: object
  properties:
    result:
      type: string
---

# Node Test Skill

A simple Node.js skill for testing the sandbox.
