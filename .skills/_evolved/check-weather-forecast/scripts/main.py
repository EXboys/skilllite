#!/usr/bin/env python3
"""查询指定城市未来几天的天气预报。使用 wttr.in API。"""
import sys
import json
import urllib.request
import urllib.error
import urllib.parse

def main():
    try:
        input_data = json.load(sys.stdin)
        city = input_data.get("city", "深圳")
        day_offset = input_data.get("day_offset", 1)

        try:
            day_offset = int(day_offset)
        except (TypeError, ValueError):
            day_offset = 1

        if day_offset < 0 or day_offset > 7:
            day_offset = min(max(day_offset, 0), 7)

        encoded = urllib.parse.quote(city)
        url = f"https://wttr.in/{encoded}?format=j1&lang=zh"
        headers = {"User-Agent": "curl/7.64.1"}
        req = urllib.request.Request(url, headers=headers)

        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode("utf-8"))

        weather_list = data.get("weather", [])
        if day_offset >= len(weather_list):
            output = {"error": f"无法获取 {day_offset} 天后的预报", "city": city}
        else:
            day = weather_list[day_offset]
            maxtemp = day.get("maxtempC", "N/A")
            mintemp = day.get("mintempC", "N/A")
            desc = day.get("lang_zh", [{}])[0].get("value", "未知") if day.get("lang_zh") else "未知"
            output = {
                "city": city,
                "date": day.get("date", ""),
                "weather": desc,
                "high": f"{maxtemp}°C",
                "low": f"{mintemp}°C",
                "day_offset": day_offset,
                "source": "wttr.in",
                "success": True,
            }

        json.dump(output, sys.stdout, ensure_ascii=False)
        sys.stdout.write("\n")

    except json.JSONDecodeError:
        json.dump({"error": "Invalid JSON input."}, sys.stderr, ensure_ascii=False)
        sys.stderr.write("\n")
        sys.exit(1)
    except ValueError as ve:
        json.dump({"error": str(ve)}, sys.stderr, ensure_ascii=False)
        sys.stderr.write("\n")
        sys.exit(1)
    except urllib.error.URLError as e:
        json.dump({"error": f"网络请求失败: {e.reason}"}, sys.stderr, ensure_ascii=False)
        sys.stderr.write("\n")
        sys.exit(1)
    except Exception as e:
        json.dump({"error": str(e)}, sys.stderr, ensure_ascii=False)
        sys.stderr.write("\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
