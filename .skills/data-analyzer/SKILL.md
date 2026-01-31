---
name: data-analyzer
description: 数据分析工具，使用 pandas 进行数据统计分析。当用户需要对数据进行统计分析、计算均值、求和、描述性统计时使用。
license: MIT
compatibility: Requires Python 3.x with pandas and numpy
metadata:
  author: skillLite
  version: "1.0"
---

# Data Analyzer Skill

使用 pandas 进行数据统计分析的技能。

## 功能特性

- 支持 JSON 格式数据输入
- 支持多种统计分析操作：
  - `describe` - 描述性统计（均值、标准差、最小值、最大值等）
  - `mean` - 计算数值列均值
  - `sum` - 计算数值列求和
  - `count` - 统计行数和列信息

## 使用示例

### 描述性统计
```json
{
  "data": "[{\"name\": \"Alice\", \"age\": 25, \"score\": 85}, {\"name\": \"Bob\", \"age\": 30, \"score\": 92}]",
  "operation": "describe"
}
```

### 计算均值
```json
{
  "data": "[{\"value\": 10}, {\"value\": 20}, {\"value\": 30}]",
  "operation": "mean"
}
```

## Runtime

```yaml
entry_point: scripts/main.py
language: python
dependencies:
  - pandas>=2.0.0
  - numpy>=1.24.0
input_schema:
  type: object
  properties:
    data:
      type: string
      description: "JSON 格式的数据，如: [{\"name\": \"Alice\", \"age\": 25}, {\"name\": \"Bob\", \"age\": 30}]"
    operation:
      type: string
      description: 分析操作类型
      enum:
        - describe
        - mean
        - sum
        - count
      default: describe
  required:
    - data
```
