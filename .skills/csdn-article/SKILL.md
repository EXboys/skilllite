---
name: csdn-article
description: CSDN 博客文章格式与风格指引。当用户要求写 CSDN 文章时，按此格式直接输出 Markdown，无需调用工具。
license: MIT
metadata:
  author: skillLite
  version: "1.0"
---

# CSDN 文章格式指引

当用户要求写 CSDN 文章、博客时，**直接在回复中输出完整的 Markdown**，无需调用任何工具。同时参考 writing-helper 去 AI 味。

---

## 输出格式（必须遵守）

### 结构
1. **一级标题**：`# 文章标题`（含关键词，便于 SEO）
2. **摘要**：用 `> 摘要内容` 引用块，100–150 字概括核心
3. **正文**：分段清晰，使用 `##`、`###` 组织层级
4. **代码块**：使用 ` ```language ` 指定语言（如 ` ```rust `、` ```python `）
5. **文末标签**（可选）：`---` 后跟 `#标签1 #标签2`

### 示例骨架
```markdown
# Rust 异步编程入门：从 Future 到 async/await

> 本文介绍 Rust 异步编程基础，包括 Future、async/await 语法，以及常见使用场景。

## 为什么需要异步
...

## 基本用法
```rust
async fn main() {}
```
...

---
#Rust #异步编程
```

---

## 技术类文章要点

- **标题**：信息密度高，可带悬念，避免标题党
- **摘要**：让读者 5 秒内知道能学到什么
- **正文**：技术准确、逻辑清晰；代码示例可运行
- **风格**：参考 writing-helper，去 AI 味、有具体细节、像人在说话
