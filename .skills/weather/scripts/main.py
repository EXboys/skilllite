#!/usr/bin/env python3
"""
天气查询 Skill - 使用真实天气 API
支持的 API（按优先级）：
1. 中华万年历 - 免费，无需 API Key，国内稳定
2. sojson天气 - 免费，无需 API Key，含空气质量
3. 和风天气 (QWeather) - 需要 API Key
4. 高德天气 (Amap) - 需要 API Key
5. wttr.in - 免费无需 Key（国外服务，可能超时）
"""
import json
import os
import sys
import ssl
import urllib.request
import urllib.parse
import urllib.error

# API Key 配置（可选）
QWEATHER_API_KEY = os.environ.get("QWEATHER_API_KEY", "")
AMAP_API_KEY = os.environ.get("AMAP_API_KEY", "")

# 创建不验证 SSL 的上下文（用于某些网络环境）
SSL_CONTEXT = ssl.create_default_context()
SSL_CONTEXT.check_hostname = False
SSL_CONTEXT.verify_mode = ssl.CERT_NONE

# 城市名到城市编码的映射（常用城市）
CITY_CODE_MAP = {
    "qweather": {
        "北京": "101010100", "上海": "101020100", "深圳": "101280601",
        "广州": "101280101", "杭州": "101210101", "成都": "101270101",
        "武汉": "101200101", "西安": "101110101", "南京": "101190101",
        "重庆": "101040100", "天津": "101030100", "苏州": "101190401",
        "厦门": "101230201", "青岛": "101120201", "大连": "101070201",
        "长沙": "101250101", "郑州": "101180101", "济南": "101120101",
        "沈阳": "101070101", "哈尔滨": "101050101",
    },
    "amap": {
        "北京": "110000", "上海": "310000", "深圳": "440300",
        "广州": "440100", "杭州": "330100", "成都": "510100",
        "武汉": "420100", "西安": "610100", "南京": "320100",
        "重庆": "500000", "天津": "120000", "苏州": "320500",
        "厦门": "350200", "青岛": "370200", "大连": "210200",
        "长沙": "430100", "郑州": "410100", "济南": "370100",
        "沈阳": "210100", "哈尔滨": "230100", "福州": "350100",
        "合肥": "340100", "昆明": "530100", "贵阳": "520100",
        "南宁": "450100", "海口": "460100", "太原": "140100",
        "石家庄": "130100", "兰州": "620100", "银川": "640100",
        "西宁": "630100", "拉萨": "540100", "乌鲁木齐": "650100",
        "呼和浩特": "150100", "长春": "220100", "南昌": "360100",
    }
}


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
    使用 sojson 天气 API（免费，无需 API Key，国内稳定）
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


def get_weather_from_amap(city: str) -> dict:
    """
    使用高德天气 API 获取天气（国内访问稳定）
    免费注册: https://lbs.amap.com （每日免费 30 万次调用）
    """
    if not AMAP_API_KEY:
        return {"error": "未配置 AMAP_API_KEY", "success": False}
    
    try:
        adcode = CITY_CODE_MAP["amap"].get(city)
        
        if not adcode:
            geo_url = f"https://restapi.amap.com/v3/geocode/geo?address={urllib.parse.quote(city)}&key={AMAP_API_KEY}"
            geo_result = make_request(geo_url)
            if not geo_result["success"]:
                return {"city": city, "error": geo_result["error"], "success": False}
            
            geo_data = geo_result["data"]
            if geo_data.get("status") != "1" or not geo_data.get("geocodes"):
                return {"city": city, "error": f"未找到城市: {city}", "success": False}
            
            adcode = geo_data["geocodes"][0]["adcode"]
        
        weather_url = f"https://restapi.amap.com/v3/weather/weatherInfo?city={adcode}&key={AMAP_API_KEY}&extensions=base"
        weather_result = make_request(weather_url)
        if not weather_result["success"]:
            return {"city": city, "error": weather_result["error"], "success": False}
        
        weather_data = weather_result["data"]
        if weather_data.get("status") != "1" or not weather_data.get("lives"):
            return {"city": city, "error": "获取天气失败", "success": False}
        
        live = weather_data["lives"][0]
        return {
            "city": city,
            "province": live.get("province", ""),
            "temperature": f"{live.get('temperature', 'N/A')}°C",
            "weather": live.get("weather", "未知"),
            "humidity": f"{live.get('humidity', 'N/A')}%",
            "wind": f"{live.get('winddirection', '')}风 {live.get('windpower', '')}级",
            "report_time": live.get("reporttime", ""),
            "source": "高德天气",
            "success": True
        }
    except Exception as e:
        return {"city": city, "error": str(e), "success": False}


def get_weather_from_qweather(city: str) -> dict:
    """
    使用和风天气 API 获取天气（需要 API Key）
    免费注册: https://dev.qweather.com
    """
    if not QWEATHER_API_KEY:
        return {"error": "未配置 QWEATHER_API_KEY", "success": False}
    
    try:
        location_id = CITY_CODE_MAP["qweather"].get(city)
        
        if not location_id:
            geo_url = f"https://geoapi.qweather.com/v2/city/lookup?location={urllib.parse.quote(city)}&key={QWEATHER_API_KEY}"
            geo_result = make_request(geo_url)
            if not geo_result["success"]:
                return {"city": city, "error": geo_result["error"], "success": False}
            
            geo_data = geo_result["data"]
            if geo_data.get("code") != "200" or not geo_data.get("location"):
                return {"city": city, "error": f"未找到城市: {city}", "success": False}
            
            location_id = geo_data["location"][0]["id"]
        
        weather_url = f"https://devapi.qweather.com/v7/weather/now?location={location_id}&key={QWEATHER_API_KEY}"
        weather_result = make_request(weather_url)
        if not weather_result["success"]:
            return {"city": city, "error": weather_result["error"], "success": False}
        
        weather_data = weather_result["data"]
        if weather_data.get("code") != "200":
            return {"city": city, "error": f"获取天气失败: {weather_data.get('code')}", "success": False}
        
        now = weather_data.get("now", {})
        return {
            "city": city,
            "temperature": f"{now.get('temp', 'N/A')}°C",
            "feels_like": f"{now.get('feelsLike', 'N/A')}°C",
            "weather": now.get("text", "未知"),
            "humidity": f"{now.get('humidity', 'N/A')}%",
            "wind": f"{now.get('windDir', '')} {now.get('windScale', '')}级",
            "wind_speed": f"{now.get('windSpeed', 0)}km/h",
            "visibility": f"{now.get('vis', 0)}km",
            "pressure": f"{now.get('pressure', 0)}hPa",
            "update_time": weather_data.get("updateTime", ""),
            "source": "和风天气",
            "success": True
        }
    except Exception as e:
        return {"city": city, "error": str(e), "success": False}


def get_weather_from_wttr(city: str) -> dict:
    """
    使用 wttr.in 获取天气（免费，无需 API Key，但国内可能超时）
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
    优先级：中华万年历（免费）> sojson（免费）> 和风天气 > 高德天气 > wttr.in
    """
    errors = []
    
    # 1. 优先使用免费的中华万年历 API（国内稳定，无需 Key）
    result = get_weather_from_wnl(city)
    if result.get("success"):
        return result
    errors.append(f"中华万年历: {result.get('error', '未知错误')}")
    
    # 2. 备用免费 sojson API
    result = get_weather_from_sojson(city)
    if result.get("success"):
        return result
    errors.append(f"sojson: {result.get('error', '未知错误')}")
    
    # 3. 和风天气（需要 Key）
    if QWEATHER_API_KEY:
        result = get_weather_from_qweather(city)
        if result.get("success"):
            return result
        errors.append(f"和风天气: {result.get('error', '未知错误')}")
    
    # 4. 高德天气（需要 Key）
    if AMAP_API_KEY:
        result = get_weather_from_amap(city)
        if result.get("success"):
            return result
        errors.append(f"高德天气: {result.get('error', '未知错误')}")
    
    # 5. wttr.in（国外服务，可能超时）
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