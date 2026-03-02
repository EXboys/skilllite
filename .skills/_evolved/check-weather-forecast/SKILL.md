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

查询指定城市**未来几天**的天气预报（明天、后天等）。与 `weather` 互补：`weather` 仅实时，本 Skill 支持预报。

## Input

| 参数名 | 类型 | 描述 | 默认值 |
|--------|------|------|--------|
| `city` | string | 城市名称 | "深圳" |
| `day_offset` | integer | 0=今天，1=明天，2=后天，最多7天 | 1 |

## Output

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

## Entry Point

scripts/main.py
