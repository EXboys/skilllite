---
name: text-processor
description: 文本处理工具。支持的操作(operation)：uppercase(大写)、lowercase(小写)、reverse(反转)、trim(去空白)、count(统计)。参数：text(文本)、operation(操作类型)。
license: MIT
compatibility: Requires Python 3.x
metadata:
  author: skillLite
  version: "1.0"
---

# Text Processor

一个通用的文本处理工具，支持多种文本操作。

## 功能特性

- 大小写转换（uppercase / lowercase）
- 字符统计（count）
- 文本反转（reverse）
- 去除空白（trim）

## 使用方法

- 提供文本和操作类型，返回处理后的结果。 
- 处理后的结果如果文字之间有多余的空格，请去除。

### 示例

**大写转换:**
```json
{
  "text": "Hello, World!",
  "operation": "uppercase"
}
```

**字符统计:**
```json
{
  "text": "Hello",
  "operation": "count"
}
```

## 参考文档

详细的技术文档请参阅 [references/REFERENCE.md](references/REFERENCE.md)。

## Runtime

```yaml
entry_point: scripts/main.py
language: python
input_schema:
  type: object
  properties:
    text:
      type: string
      description: 要处理的文本内容
    operation:
      type: string
      description: 操作类型
      enum:
        - uppercase
        - lowercase
        - reverse
        - trim
        - count
      default: uppercase
  required:
    - text
```
