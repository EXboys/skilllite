---
name: weather
description: 查询城市天气信息，支持查询今天和明天的天气预报。当用户询问某个城市的天气、温度、湿度等信息时使用。
license: MIT
compatibility: Requires Python 3.x, network access
metadata:
  author: skillLite
  version: "3.2"
---

# Weather Skill

查询指定城市的**真实天气**信息。**开箱即用，无需配置任何 API Key！**

## 数据来源（按优先级，均免费无需 Key）

| 优先级 | 数据源 | 说明 |
|--------|--------|------|
| 1 | **中华万年历** | 国内免费稳定，默认使用 |
| 2 | **sojson天气** | 备用免费源，含空气质量 |
| 3 | **wttr.in** | 仅作「今天」的兜底；国外服务，可能超时（明天及以后依赖前两个源） |

## 参数

- `city` (string, required): 城市名称，如 '北京'、'深圳'、'清迈'
- `day` (string, optional): 查询哪天的天气，可选值: 'today' (今天, 默认), 'tomorrow' (明天)

## 示例

**查询今天天气**

输入: `{"city": "深圳", "day": "today"}`
输出:
```json
{
  "city": "深圳",
  "temperature": "18°C",
  "weather": "多云",
  "high": "20°C",
  "low": "14°C",
  "wind": "东北风 3-4级",
  "tip": "天气较凉，注意添加衣物",
  "source": "中华万年历",
  "success": true
}
```

**查询明天天气**

输入: `{"city": "深圳", "day": "tomorrow"}`
输出:
```json
{
  "city": "深圳",
  "temperature": "18°C",
  "weather": "多云",
  "high": "20°C",
  "low": "14°C",
  "wind": "东北风 3-4级",
  "tip": "注意防晒",
  "source": "中华万年历",
  "success": true
}
```
