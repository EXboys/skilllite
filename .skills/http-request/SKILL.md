---
name: http-request
description: 发起 HTTP 网络请求，支持 GET、POST、PUT、DELETE、PATCH 方法。当用户需要调用 API、获取网页内容、发送数据到服务器时使用。
license: MIT
compatibility: Requires Python 3.x with requests library, network access
metadata:
  author: skillLite
  version: "1.0"
---

# HTTP Request Skill

一个通用的 HTTP 网络请求技能，支持常见的 RESTful API 调用。

## 功能特性

- 支持 GET、POST、PUT、DELETE、PATCH 方法
- 支持自定义请求头
- 支持 JSON 请求体
- 支持 URL 查询参数
- 支持超时设置

## 使用示例

### GET 请求
```json
{
  "url": "https://httpbin.org/get",
  "method": "GET",
  "params": {"name": "test"}
}
```

### POST 请求
```json
{
  "url": "https://httpbin.org/post",
  "method": "POST",
  "headers": {"Content-Type": "application/json"},
  "body": {"message": "hello world"}
}
```

## Runtime

```yaml
entry_point: scripts/main.py
language: python
network:
  enabled: true
  outbound:
    - "*:80"
    - "*:443"
  block_private_ips: false
input_schema:
  type: object
  properties:
    url:
      type: string
      description: 请求的完整 URL
    method:
      type: string
      description: HTTP 请求方法
      enum:
        - GET
        - POST
        - PUT
        - DELETE
        - PATCH
      default: GET
    headers:
      type: object
      description: 自定义请求头
      additionalProperties:
        type: string
    body:
      type: object
      description: 请求体数据，用于 POST/PUT/PATCH，将以 JSON 格式发送
    params:
      type: object
      description: URL 查询参数
      additionalProperties: true
    timeout:
      type: number
      description: 请求超时时间（秒），默认 30 秒
      default: 30
  required:
    - url
```
