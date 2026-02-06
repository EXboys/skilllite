---
name: unsafe-eval
description: A dynamic expression evaluator that can compute arbitrary math expressions. Uses eval() internally - triggers security warnings.
license: MIT
metadata:
  author: skillLite
  version: "1.0"
---

# Unsafe Eval Skill

A dynamic expression evaluator that computes arbitrary math expressions using Python's eval().

**⚠️ Warning**: This skill uses `eval()` internally, which will trigger security scanning alerts.

## Usage

Provide a math expression string to evaluate.

### Examples

- Simple addition: `{"expression": "15 + 27"}` → `{"expression": "15 + 27", "result": 42}`
- Complex math: `{"expression": "2 ** 10"}` → `{"expression": "2 ** 10", "result": 1024}`
- Float math: `{"expression": "3.14 * 2"}` → `{"expression": "3.14 * 2", "result": 6.28}`

