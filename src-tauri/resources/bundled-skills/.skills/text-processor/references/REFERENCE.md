# Text Processor 技术参考文档

本文档提供 Text Processor 的详细技术说明。

## 输入参数

| 参数 | 类型 | 必需 | 默认值 | 说明 |
|------|------|------|--------|------|
| text | string | 是 | - | 要处理的文本内容 |
| operation | string | 否 | uppercase | 操作类型 |

## 支持的操作

| 操作 | 说明 | 示例输出 |
|------|------|----------|
| uppercase | 转换为大写 | "HELLO" |
| lowercase | 转换为小写 | "hello" |
| reverse | 反转文本 | "olleH" |
| trim | 去除首尾空白 | "Hello" |
| count | 统计字符信息 | {"length": 5, "words": 1, ...} |

## 输出格式

### 文本转换响应

```json
{
  "success": true,
  "original": "Hello",
  "operation": "uppercase",
  "processed": "HELLO"
}
```

### 统计响应

```json
{
  "success": true,
  "original": "Hello World",
  "operation": "count",
  "statistics": {
    "length": 11,
    "words": 2,
    "lines": 1,
    "chars_no_space": 10
  }
}
```

### 错误响应

```json
{
  "success": false,
  "error": "错误描述"
}
```

## 错误处理

| 错误类型 | 说明 |
|----------|------|
| Text is required | 未提供 text 参数 |
| Unknown operation | 不支持的操作类型 |
| Invalid JSON input | 输入不是有效的 JSON 格式 |
