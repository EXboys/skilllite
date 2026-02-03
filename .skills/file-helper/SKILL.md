---
name: file-helper
description: 文件助手工具，帮助用户读取和处理文件内容。当用户需要查看文件、读取文本内容时使用。
license: MIT
metadata:
  author: skilllite
  version: "1.0"
---

# File Helper

A helpful tool for reading and processing files.

## Runtime

```yaml
input_schema:
  type: object
  properties:
    filepath:
      type: string
      description: 要读取的文件路径
    action:
      type: string
      description: 操作类型 (read, list, info)
      default: read
  required: [filepath]
```

