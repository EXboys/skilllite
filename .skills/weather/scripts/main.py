#!/usr/bin/env python3
"""
天气查询 Skill - 使用真实天气 API
支持的 API（按优先级，均免费无需 API Key）：
1. 中华万年历 - 国内稳定
2. sojson天气 - 含空气质量
3. wttr.in - 国外服务，可能超时
"""
import json
import sys
import ssl
import urllib.request
import urllib.parse
import urllib.error

# 创建不验证 SSL 的上下文（用于某些网络环境）
SSL_CONTEXT = ssl.create_default_context()
SSL_CONTEXT.check_hostname = False
SSL_CONTEXT.verify_mode = ssl.CERT_NONE


def make_request(url: str, timeout: int = 5, headers: dict = None) -> dict:
    """发起 HTTP 请求，带超时和 SSL 容错"""
    try:
        default_headers = {"User-Agent": "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X)"}
        if headers:
            default_headers.update(headers)
        request = urllib.request.Request(url, headers=default_headers)
        with urllib.request.urlopen(request, timeout=timeout, context=SSL_CONTEXT) as response:
            return {"data": json.loads(response.read().decode("utf-8")), "success": True}
    except Exception as e:
        return {"error": str(e), "success": False}


def get_weather_from_wnl(city: str) -> dict:
    """
    使用中华万年历天气 API（免费，无需 API Key，国内稳定）
    """
    try:
        url = f"https://wthrcdn.etouch.cn/weather_mini?city={urllib.parse.quote(city)}"
        result = make_request(url, timeout=5)
        
        if not result["success"]:
            return {"city": city, "error": result["error"], "success": False}
        
        data = result["data"]
        if data.get("status") != 1000 or not data.get("data"):
            desc = data.get("desc", "未知错误")
            return {"city": city, "error": f"获取失败: {desc}", "success": False}
        
        weather_data = data["data"]
        today = weather_data.get("forecast", [{}])[0] if weather_data.get("forecast") else {}
        
        return {
            "city": weather_data.get("city", city),
            "temperature": weather_data.get("wendu", "N/A") + "°C",
            "weather": today.get("type", "未知"),
            "high": today.get("high", "").replace("高温 ", ""),
            "low": today.get("low", "").replace("低温 ", ""),
            "wind": f"{today.get('fengxiang', '')} {today.get('fengli', '').replace('<![CDATA[', '').replace(']]>', '')}",
            "tip": weather_data.get("ganmao", ""),
            "source": "中华万年历",
            "success": True
        }
    except Exception as e:
        return {"city": city, "error": str(e), "success": False}


def get_weather_from_sojson(city: str) -> dict:
    """
    使用 sojson 天气 API（免费，无需 API Key，含空气质量）
    """
    city_code_map = {
        "北京": "101010100", "上海": "101020100", "深圳": "101280601",
        "广州": "101280101", "杭州": "101210101", "成都": "101270101",
        "武汉": "101200101", "西安": "101110101", "南京": "101190101",
        "重庆": "101040100", "天津": "101030100", "苏州": "101190401",
        "厦门": "101230201", "青岛": "101120201", "大连": "101070201",
        "长沙": "101250101", "郑州": "101180101", "济南": "101120101",
        "沈阳": "101070101", "哈尔滨": "101050101", "福州": "101230101",
        "合肥": "101220101", "昆明": "101290101", "贵阳": "101260101",
        "南宁": "101300101", "海口": "101310101", "太原": "101100101",
        "石家庄": "101090101", "兰州": "101160101", "银川": "101170101",
        "西宁": "101150101", "拉萨": "101140101", "乌鲁木齐": "101130101",
        "呼和浩特": "101080101", "长春": "101060101", "南昌": "101240101",
        "珠海": "101280701", "东莞": "101281601", "佛山": "101280800",
        "中山": "101281701", "惠州": "101280301", "汕头": "101280501",
    }
    try:
        city_code = city_code_map.get(city)
        if not city_code:
            return {"city": city, "error": f"暂不支持城市: {city}，请尝试省会城市", "success": False}
        url = f"http://t.weather.sojson.com/api/weather/city/{city_code}"
        result = make_request(url, timeout=5)
        if not result["success"]:
            return {"city": city, "error": result["error"], "success": False}
        data = result["data"]
        if data.get("status") != 200:
            return {"city": city, "error": data.get("message", "获取失败"), "success": False}
        city_info = data.get("cityInfo", {})
        weather_data = data.get("data", {})
        today = weather_data.get("forecast", [{}])[0] if weather_data.get("forecast") else {}
        return {
            "city": city_info.get("city", city),
            "temperature": weather_data.get("wendu", "N/A") + "°C",
            "humidity": weather_data.get("shidu", "N/A"),
            "weather": today.get("type", "未知"),
            "high": today.get("high", "").replace("高温 ", ""),
            "low": today.get("low", "").replace("低温 ", ""),
            "wind": f"{today.get('fx', '')} {today.get('fl', '')}",
            "air_quality": f"PM2.5: {weather_data.get('pm25', 'N/A')} 空气质量: {weather_data.get('quality', 'N/A')}",
            "tip": today.get("notice", ""),
            "update_time": data.get("time", ""),
            "source": "sojson天气",
            "success": True
        }
    except Exception as e:
        return {"city": city, "error": str(e), "success": False}


def get_weather_from_wttr(city: str) -> dict:
    """
    使用 wttr.in 获取天气（免费，无需 API Key，国外服务可能超时）
    """
    try:
        encoded_city = urllib.parse.quote(city)
        url = f"https://wttr.in/{encoded_city}?format=j1&lang=zh"
        result = make_request(url, timeout=8)
        if not result["success"]:
            return {"city": city, "error": result["error"], "success": False}
        data = result["data"]
        current = data.get("current_condition", [{}])[0]
        weather_desc = current.get("lang_zh", [{}])
        weather_text = weather_desc[0].get("value", "未知") if weather_desc else current.get("weatherDesc", [{}])[0].get("value", "未知")
        return {
            "city": city,
            "temperature": f"{current.get('temp_C', 'N/A')}°C",
            "feels_like": f"{current.get('FeelsLikeC', 'N/A')}°C",
            "weather": weather_text,
            "humidity": f"{current.get('humidity', 'N/A')}%",
            "wind": f"{current.get('winddir16Point', '')} {current.get('windspeedKmph', 0)}km/h",
            "visibility": f"{current.get('visibility', 0)}km",
            "uv_index": current.get("uvIndex", "N/A"),
            "source": "wttr.in",
            "success": True
        }
    except Exception as e:
        return {"city": city, "error": str(e), "success": False}


def get_weather(city: str) -> dict:
    """
    获取城市天气
    优先级：中华万年历 > sojson > wttr.in（均免费无需 API Key）
    """
    errors = []
    result = get_weather_from_wnl(city)
    if result.get("success"):
        return result
    errors.append(f"中华万年历: {result.get('error', '未知错误')}")
    result = get_weather_from_sojson(city)
    if result.get("success"):
        return result
    errors.append(f"sojson: {result.get('error', '未知错误')}")
    result = get_weather_from_wttr(city)
    if result.get("success"):
        return result
    errors.append(f"wttr.in: {result.get('error', '未知错误')}")
    return {
        "city": city,
        "error": "所有天气源均获取失败",
        "details": errors,
        "tip": "请检查网络连接或城市名称是否正确",
        "success": False
    }


def main():
    input_data = json.loads(sys.stdin.read())
    city = input_data.get("city", "北京")
    
    result = get_weather(city)
    
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()