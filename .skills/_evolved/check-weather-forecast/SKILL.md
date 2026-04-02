---
name: check-weather-forecast
description: 查询指定城市未来几天（明天、后天等）的天气预报。当用户询问「明天」「后天」「未来几天」天气时使用；与 weather 互补，weather 仅提供实时天气。
license: MIT
compatibility: Requires Python 3.x, network access
metadata:
  author: skillLite-evolution
  version: "1.0"
---

# Skill: check-weather-forecast

查询指定城市**未来几天**的天气预报（明天、后天等）。与 `weather` 互补：`weather` 仅提供实时天气，本 Skill 支持预报。

## scripts/main.py

查询指定城市未来几天的天气预报。该脚本通过标准输入接收 JSON 参数，并输出 JSON 格式的天气预报结果。

### Input Schema

| 参数名 | 类型 | 描述 | 默认值 |
|--------|------|------|--------|
| `city` | string | 城市名称 | "深圳" |
| `day_offset` | integer | 0=今天，1=明天，2=后天，最多7天 | 1 |

### Output Schema

```json
{
  "city": "深圳",
  "date": "2026-03-03",
  "weather": "多云",
  "high": "28°C",
  "low": "21°C",
  "day_offset": 1,
  "source": "wttr.in",
  "success": true
}
```

## Usage

要运行 `scripts/main.py`，请通过标准输入提供 JSON 数据。

```bash
echo '{"city": "北京", "day_offset": 2}' | python3 scripts/main.py
```

## Examples

### Example 1: 查询深圳明天的天气

**Input:**

```json
{
  "city": "深圳",
  "day_offset": 1
}
```

**Output (示例，实际结果可能因时间而异):**

```json
{
  "city": "深圳",
  "date": "2024-07-20",
  "weather": "多云",
  "high": "32°C",
  "low": "26°C",
  "day_offset": 1,
  "source": "wttr.in",
  "success": true
}
```
