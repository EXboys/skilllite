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

## 数据来源（按优先级）

| 优先级 | 数据源 | 需要 Key | 说明 |
|--------|--------|----------|------|
| 1 | **中华万年历** | ❌ 不需要 | 国内免费稳定，默认使用 |
| 2 | **sojson天气** | ❌ 不需要 | 备用免费源，含空气质量 |
| 3 | 和风天气 | ✅ 需要 | 数据详细 |
| 4 | 高德天气 | ✅ 需要 | 国内稳定 |
| 5 | wttr.in | ❌ 不需要 | 国外服务，可能超时 |

## 直接使用（推荐）

无需任何配置，直接查询即可获取真实天气数据！

## 可选配置

如需更详细的天气数据，可配置以下 API：

### 高德天气 API
```bash
export AMAP_API_KEY=your_key  # https://lbs.amap.com
```

### 和风天气 API
```bash
export QWEATHER_API_KEY=your_key  # https://dev.qweather.com
```

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


