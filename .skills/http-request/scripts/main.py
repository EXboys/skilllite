#!/usr/bin/env python3
"""
HTTP Request Skill - 发起 HTTP 网络请求
优化版：支持旧版 SSL 服务器、浏览器级 User-Agent、重试逻辑、HTML→Markdown 转换
"""

import json
import re
import sys
import ssl

# html2text：HTML 转 Markdown，默认用于网页内容，降低 token 消耗
try:
    import html2text
    HAS_HTML2TEXT = True
except ImportError:
    HAS_HTML2TEXT = False

# 优先使用 requests（更稳健），fallback 到 urllib
try:
    import requests
    from requests.adapters import HTTPAdapter
    from urllib3.util.retry import Retry
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False
    import urllib.request
    import urllib.parse
    import urllib.error


# 浏览器级 User-Agent，减少 503/反爬拦截
DEFAULT_USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
)

# 默认请求头，模拟真实浏览器
DEFAULT_HEADERS = {
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.9",
}


def _looks_like_html(body: str, headers: dict) -> bool:
    """判断响应是否为 HTML"""
    if not body or not isinstance(body, str):
        return False
    ct = ""
    for k, v in (headers or {}).items():
        if k.lower() == "content-type":
            ct = str(v).lower()
            break
    if "text/html" in ct or "application/xhtml" in ct:
        return True
    stripped = body.strip()
    return stripped.startswith("<!") or stripped.lower().startswith("<html")


def _convert_html(body: str, extract_mode: str) -> str:
    """将 HTML 转为 Markdown 或纯文本"""
    if extract_mode == "raw":
        return body
    if extract_mode == "text":
        # 纯文本：去除标签
        return re.sub(r"<[^>]+>", "", body).strip()
    # extract_mode == "markdown"（默认）
    if HAS_HTML2TEXT:
        h = html2text.HTML2Text()
        h.ignore_links = False
        h.ignore_images = False
        h.body_width = 0  # 不自动换行
        return h.handle(body).strip()
    # 无 html2text 时退化为纯文本
    return re.sub(r"<[^>]+>", "", body).strip()


def _create_ssl_context_legacy():
    """创建支持旧版 SSL 服务器的上下文（解决 UNSAFE_LEGACY_RENEGOTIATION_DISABLED）"""
    ctx = ssl.create_default_context(ssl.Purpose.SERVER_AUTH)
    # OP_LEGACY_SERVER_CONNECT = 0x4，允许连接不支持 RFC 5746 安全重协商的旧服务器
    if hasattr(ssl, "OP_LEGACY_SERVER_CONNECT"):
        ctx.options |= ssl.OP_LEGACY_SERVER_CONNECT
    else:
        ctx.options |= 0x4  # OpenSSL 常量，兼容旧版 Python
    return ctx


def _create_http_adapter(use_legacy_ssl=True):
    """创建 HTTP Adapter：支持旧版 SSL + 重试"""
    class LegacySSLAdapter(HTTPAdapter):
        def init_poolmanager(self, *args, **kwargs):
            if use_legacy_ssl:
                kwargs["ssl_context"] = _create_ssl_context_legacy()
            return super().init_poolmanager(*args, **kwargs)

    retry = Retry(
        total=2,
        backoff_factor=1,
        status_forcelist=[502, 503, 504],
        allowed_methods=["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"],
    )
    return LegacySSLAdapter(max_retries=retry)


def _request_with_requests(input_data):
    """使用 requests 库发起请求"""
    url = input_data.get("url")
    method = input_data.get("method", "GET").upper()
    headers = dict(input_data.get("headers", {}))
    body = input_data.get("body")
    params = input_data.get("params", {})
    timeout = input_data.get("timeout", 30)
    use_legacy_ssl = input_data.get("use_legacy_ssl", True)  # 默认启用，兼容旧服务器

    if "User-Agent" not in headers:
        headers["User-Agent"] = DEFAULT_USER_AGENT
    for k, v in DEFAULT_HEADERS.items():
        if k not in headers:
            headers[k] = v

    session = requests.Session()
    adapter = _create_http_adapter(use_legacy_ssl=use_legacy_ssl)
    session.mount("https://", adapter)
    session.mount("http://", adapter)

    try:
        if body is not None and method in ["POST", "PUT", "PATCH"]:
            resp = session.request(
                method, url, json=body, params=params, headers=headers, timeout=timeout
            )
        else:
            resp = session.request(
                method, url, params=params, headers=headers, timeout=timeout
            )

        # 尝试解析 JSON
        try:
            response_json = resp.json()
            body_out = response_json
        except Exception:
            body_out = resp.text

        # 4xx/5xx 视为失败（与旧版行为一致）
        if resp.status_code >= 400:
            return {
                "success": False,
                "status_code": resp.status_code,
                "error": resp.reason or f"HTTP {resp.status_code}",
                "body": body_out,
            }
        return {
            "success": True,
            "status_code": resp.status_code,
            "headers": dict(resp.headers),
            "body": body_out,
            "is_json": isinstance(body_out, dict),
        }

    except requests.exceptions.SSLError as e:
        # SSL 错误时，若未用 legacy，可提示尝试 use_legacy_ssl
        return {
            "success": False,
            "error": f"SSL Error: {str(e)}",
            "hint": "部分旧版服务器需 use_legacy_ssl: true（已默认启用）",
        }
    except requests.exceptions.ConnectionError as e:
        err_msg = str(e)
        if "nodename nor servname" in err_msg or "Name or service not known" in err_msg:
            return {"success": False, "error": "DNS 解析失败，请检查域名或网络"}
        return {"success": False, "error": f"Connection Error: {err_msg}"}
    except requests.exceptions.Timeout:
        return {"success": False, "error": f"请求超时（{timeout} 秒）"}
    except requests.exceptions.RequestException as e:
        return {"success": False, "error": str(e)}


def _request_with_urllib(input_data):
    """使用 urllib 发起请求（无 requests 时的 fallback）"""
    url = input_data.get("url")
    method = input_data.get("method", "GET").upper()
    headers = dict(input_data.get("headers", {}))
    body = input_data.get("body")
    params = input_data.get("params", {})
    timeout = input_data.get("timeout", 30)

    if "User-Agent" not in headers:
        headers["User-Agent"] = DEFAULT_USER_AGENT
    for k, v in DEFAULT_HEADERS.items():
        if k not in headers:
            headers[k] = v

    if params:
        from urllib.parse import urlencode, urlparse, parse_qs
        sep = "&" if "?" in url else "?"
        url = f"{url}{sep}{urlencode(params)}"

    data = None
    if body is not None and method in ["POST", "PUT", "PATCH"]:
        data = json.dumps(body).encode("utf-8")
        if "Content-Type" not in headers:
            headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    ctx = _create_ssl_context_legacy()

    try:
        with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
            resp_body = resp.read().decode("utf-8")
            try:
                body_out = json.loads(resp_body)
            except json.JSONDecodeError:
                body_out = resp_body
            return {
                "success": True,
                "status_code": resp.status,
                "headers": dict(resp.headers),
                "body": body_out,
                "is_json": isinstance(body_out, dict),
            }
    except urllib.error.HTTPError as e:
        err_body = ""
        try:
            err_body = e.read().decode("utf-8")
        except Exception:
            pass
        return {
            "success": False,
            "status_code": e.code,
            "error": str(e.reason),
            "body": err_body,
        }
    except urllib.error.URLError as e:
        err = str(e.reason)
        if "UNSAFE_LEGACY_RENEGOTIATION" in err:
            return {"success": False, "error": f"SSL 错误: {err}", "hint": "建议安装 requests 库以获得更好兼容性"}
        if "nodename nor servname" in err or "Name or service not known" in err:
            return {"success": False, "error": "DNS 解析失败，请检查域名或网络"}
        return {"success": False, "error": f"URL Error: {err}"}
    except TimeoutError:
        return {"success": False, "error": f"请求超时（{timeout} 秒）"}
    except Exception as e:
        return {"success": False, "error": str(e)}


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        print(json.dumps({"success": False, "error": f"Invalid JSON input: {e}"}))
        return

    url = input_data.get("url")
    if not url:
        print(json.dumps({"success": False, "error": "URL is required"}))
        return

    if HAS_REQUESTS:
        result = _request_with_requests(input_data)
    else:
        result = _request_with_urllib(input_data)

    # HTML 响应时转换为 Markdown（默认）或纯文本，降低 token 消耗
    extract_mode = input_data.get("extract_mode", "markdown")
    if (
        result.get("success")
        and not result.get("is_json")
        and isinstance(result.get("body"), str)
        and _looks_like_html(result["body"], result.get("headers", {}))
    ):
        result["body"] = _convert_html(result["body"], extract_mode)
        result["extract_mode"] = extract_mode

    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
