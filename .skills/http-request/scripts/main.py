#!/usr/bin/env python3
"""
HTTP Request Skill - 发起 HTTP 网络请求
"""

import json
import sys
import urllib.request
import urllib.parse
import urllib.error
import ssl


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        print(json.dumps({
            "success": False,
            "error": f"Invalid JSON input: {e}"
        }))
        return

    url = input_data.get("url")
    if not url:
        print(json.dumps({
            "success": False,
            "error": "URL is required"
        }))
        return

    method = input_data.get("method", "GET").upper()
    headers = input_data.get("headers", {})
    body = input_data.get("body")
    params = input_data.get("params", {})
    timeout = input_data.get("timeout", 30)

    # 添加查询参数到 URL
    if params:
        query_string = urllib.parse.urlencode(params)
        separator = "&" if "?" in url else "?"
        url = f"{url}{separator}{query_string}"

    # 准备请求体
    data = None
    if body is not None and method in ["POST", "PUT", "PATCH"]:
        data = json.dumps(body).encode("utf-8")
        if "Content-Type" not in headers:
            headers["Content-Type"] = "application/json"

    # 添加默认 User-Agent
    if "User-Agent" not in headers:
        headers["User-Agent"] = "AgentSkill-HTTP/1.0"

    try:
        # 创建请求
        request = urllib.request.Request(
            url,
            data=data,
            headers=headers,
            method=method
        )

        # 创建 SSL 上下文
        # 尝试使用系统证书，如果失败则使用 certifi 或禁用验证
        ssl_context = None
        try:
            ssl_context = ssl.create_default_context()
            # macOS 上可能需要手动指定证书路径
            import certifi
            ssl_context.load_verify_locations(certifi.where())
        except ImportError:
            # 如果没有 certifi，尝试使用系统默认
            try:
                ssl_context = ssl.create_default_context()
            except Exception:
                # 最后的备选：创建不验证证书的上下文（仅用于测试）
                ssl_context = ssl.create_default_context()
                ssl_context.check_hostname = False
                ssl_context.verify_mode = ssl.CERT_NONE
        except Exception:
            # 如果证书加载失败，使用不验证的上下文
            ssl_context = ssl.create_default_context()
            ssl_context.check_hostname = False
            ssl_context.verify_mode = ssl.CERT_NONE

        # 发起请求
        with urllib.request.urlopen(request, timeout=timeout, context=ssl_context) as response:
            response_body = response.read().decode("utf-8")
            response_headers = dict(response.headers)
            status_code = response.status

            # 尝试解析 JSON 响应
            try:
                response_json = json.loads(response_body)
            except json.JSONDecodeError:
                response_json = None

            result = {
                "success": True,
                "status_code": status_code,
                "headers": response_headers,
                "body": response_json if response_json is not None else response_body,
                "is_json": response_json is not None
            }

            print(json.dumps(result, ensure_ascii=False))

    except urllib.error.HTTPError as e:
        error_body = ""
        try:
            error_body = e.read().decode("utf-8")
        except Exception:
            pass

        print(json.dumps({
            "success": False,
            "status_code": e.code,
            "error": str(e.reason),
            "body": error_body
        }, ensure_ascii=False))

    except urllib.error.URLError as e:
        print(json.dumps({
            "success": False,
            "error": f"URL Error: {e.reason}"
        }, ensure_ascii=False))

    except TimeoutError:
        print(json.dumps({
            "success": False,
            "error": f"Request timed out after {timeout} seconds"
        }))

    except Exception as e:
        print(json.dumps({
            "success": False,
            "error": f"Unexpected error: {str(e)}"
        }, ensure_ascii=False))


if __name__ == "__main__":
    main()
