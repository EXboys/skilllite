---
name: weather
description: 查询城市天气信息。当用户询问某个城市的天气、温度、湿度等信息时使用。
license: MIT
compatibility: Requires Python 3.x, network access
metadata:
  author: skillLite
  version: "3.0"
---

# Weather Skill

查询指定城市的**真实天气**信息。**开箱即用，无需配置任何 API Key！**

## 数据来源（按优先级，均免费无需 Key）

| 优先级 | 数据源 | 说明 |
|--------|--------|------|
| 1 | **中华万年历** | 国内免费稳定，默认使用 |
| 2 | **sojson天气** | 备用免费源，含空气质量 |
| 3 | **wttr.in** | 国外服务，可能超时 |

## 示例

输入: `{"city": "深圳"}`
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


