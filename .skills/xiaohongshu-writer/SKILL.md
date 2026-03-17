---
name: xiaohongshu-writer
description: 小红书图文内容创作助手。**必须传入 content**（含 title、body、hashtags、thumbnail），建议 generate_thumbnail=true。优先 Playwright HTML 截图生成封面，失败时回退 Pillow。
license: MIT
compatibility: Requires Python 3.x with playwright (pip install playwright, playwright install chromium)，或 Pillow 作为回退
metadata:
  author: skillLite
  version: "1.0"
---

# 小红书图文创作助手

## 工作流程

1. **Agent 用本 Skill 的指引**，根据用户主题生成完整内容（标题、正文、标签、封面设计）
2. **调用本工具**，传入生成的 `content`，建议 `generate_thumbnail: true`
3. **脚本**：优先用 Playwright 渲染 HTML 并截图（效果好、排版一致），失败时回退 Pillow；返回 base64 及保存到 `image_path`

**无需 OpenAI**，优先 Playwright HTML 截图，回退 Pillow 绘图。

---

## 输出结构（工具返回）

```json
{
  "success": true,
  "title": "吸睛标题，带 emoji",
  "body": "正文内容",
  "hashtags": ["#话题1", "#话题2"],
  "thumbnail": {
    "cover_title": "封面显示标题",
    "accent_color": "#FF6B6B",
    "style": "gradient",
    "image_base64": "封面图 base64（仅当未保存到文件时返回，避免输出过大）",
    "image_path": "项目根目录下保存的图片路径，如 xiaohongshu_thumbnail.png",
    "image_source": "playwright 或 pillow"
  }
}
```

---

## 标题规则

- **长度**：15-25 字为宜，信息密度高
- **必备**：至少 1 个 emoji，放在开头或关键词处
- **禁止**：标题党、夸张承诺、违禁词
- **技巧**：数字+结果、反常识、疑问式、场景代入

---

## 正文规则

- **分段**：每段 2-4 行，多用空行隔开
- **语气**：口语化、像朋友分享，用"我"、"你"
- **emoji**：适当点缀，每段 0-2 个，不过度
- **结构**：开头抓人 → 干货/故事 → 总结/互动
- **禁止**：硬广感、堆砌关键词、违禁词

---

## 标签规则

- **数量**：3-5 个
- **搭配**：1 个大类话题 + 2-3 个细分 + 1 个热门
- **示例**：#生活好物 #平价好物 #宿舍党必备 #618攻略

---

## 缩略图（封面）设计

封面为**高质量图文风格**，包含三部分：**主标题**、**正文摘要**（2–5 行）、**话题标签**。由 Playwright 渲染 HTML 并截图（主）或 Pillow 绘制（备选）生成，竖版 3:4。成功后保存到项目根目录 `xiaohongshu_thumbnail.png`。

### Agent 生成 content 时，thumbnail 需包含：

| 字段 | 说明 | 示例 |
|------|------|------|
| `cover_title` | 封面上显示的主标题（可略，默认用 title） | "3 件办公室好物" |
| `accent_color` | 主色调，十六进制或中文 | "#FF6B6B" 或 "暖橙" |
| `style` | 风格 | "gradient" / "minimal" / "vibrant" |

### 风格说明

- `gradient`：渐变背景，主色到深色（默认）
- `minimal`：简约灰白
- `vibrant`：纯色块

---

## 使用方式

**工具调用**：传入 `content`（必填），即 Agent 已生成的内容。格式：

```json
{
  "content": {
    "title": "🛒 打工人的 3 件办公室好物！",
    "body": "正文...",
    "hashtags": ["#办公室好物", ...],
    "thumbnail": {
      "cover_title": "3 件办公室好物",
      "accent_color": "#FF6B6B",
      "style": "gradient"
    }
  },
  "generate_thumbnail": true
}
```

**前置条件**：
- `pip install playwright` 且执行 `playwright install chromium`（优先，skilllite init 时会安装）
- 可选回退：`pip install Pillow` 及中文字体（macOS 自带 PingFang；Linux: `apt install fonts-noto-cjk`；或于 `.skills/xiaohongshu-writer/fonts/` 放入 NotoSansCJKsc-Regular.otf）

**沙箱下使用 Playwright**：macOS 沙箱会阻止 fork/spawn。若需 HTML 截图，可设置；**仅本技能**会跳过沙箱（其它技能仍走沙箱）：
- `SKILLLITE_SANDBOX_LEVEL=2`，或
- `SKILLLITE_ALLOW_PLAYWRIGHT=1`

## Runtime

```yaml
entry_point: scripts/main.py
language: python
input_schema:
  type: object
  properties:
    content:
      type: object
      description: Agent 生成的小红书内容，含 title、body、hashtags、thumbnail
    generate_thumbnail:
      type: boolean
      description: 是否生成缩略图
      default: true
  required:
    - content
```
