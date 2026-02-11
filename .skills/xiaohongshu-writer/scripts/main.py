#!/usr/bin/env python3
"""
小红书图文创作 - 根据 Agent 生成的内容，生成封面缩略图。

优先用 Pillow 绘制（快速、无额外依赖），可选 Playwright 渲染 HTML（需 playwright install chromium）。
"""

import base64
import html
import io
import json
import os
import sys
import tempfile
from pathlib import Path


def _render_html_cover(
    title: str, body: str, hashtags: list, accent_color: str = "#FF6B6B", style: str = "gradient"
) -> str:
    """生成小红书风格图文封面 HTML。竖版 3:4，含标题、正文摘要、标签。"""
    escaped_title = html.escape(title)
    body_lines = _wrap_text(body, 18)[:5]
    escaped_body = "<br>".join(html.escape(ln) for ln in body_lines) if body_lines else ""
    escaped_tags = "  ".join(html.escape(t) for t in (hashtags or [])[:5])
    color1 = accent_color
    color2 = _darken_hex(color1, 0.3)

    if style == "minimal":
        bg = "background: linear-gradient(135deg, #f8f9fa 0%, #e9ecef 100%);"
        title_color = "#212529"
    elif style == "vibrant":
        bg = f"background: linear-gradient(135deg, {color1} 0%, {color2} 100%);"
        title_color = "#ffffff"
    else:
        # gradient (default) - 柔和渐变
        bg = f"background: linear-gradient(160deg, {color1} 0%, {color2} 50%, #2d3436 100%);"
        title_color = "#ffffff"

    return f"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    width: 1024px;
    height: 1536px;
    {bg}
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    font-family: "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
    padding: 80px 60px;
}}
.cover-title {{
    font-size: 52px;
    line-height: 1.4;
    color: {title_color};
    text-shadow: 0 2px 20px rgba(0,0,0,0.2);
    text-align: center;
    max-width: 900px;
}}
.cover-body {{
    font-size: 34px;
    line-height: 1.6;
    color: rgba(255,255,255,0.95);
    text-align: center;
    max-width: 880px;
}}
.cover-tags {{
    font-size: 26px;
    color: rgba(255,255,255,0.9);
    background: rgba(0,0,0,0.35);
    padding: 12px 24px;
    border-radius: 8px;
}}
</style>
</head>
<body>
<h1 class="cover-title">{escaped_title}</h1>
<div class="cover-body">{escaped_body}</div>
<div class="cover-tags">{escaped_tags}</div>
</body>
</html>"""


# 常见颜色名 -> 十六进制
_COLOR_MAP = {
    "暖橙": "#FF6B6B", "珊瑚": "#FF6B6B", "粉": "#FF9F9F", "粉红": "#FFB6C1",
    "薄荷": "#98D8C8", "绿": "#7BC67E", "蓝": "#6B9BD1", "紫": "#9B8FC2",
    "黄": "#F7DC6F", "米白": "#F5F5DC", "白": "#ffffff",
}


def _norm_color(val: str) -> str:
    """将颜色规范化为 #RRGGBB。"""
    if not val:
        return "#FF6B6B"
    val = val.strip()
    if val in _COLOR_MAP:
        return _COLOR_MAP[val]
    if val.startswith("#") and len(val) == 7:
        return val
    return "#FF6B6B"


def _darken_hex(hex_color: str, factor: float) -> str:
    """将十六进制颜色变暗。"""
    hex_color = hex_color.lstrip("#")
    if len(hex_color) != 6:
        return "#5a6c7d"
    r = max(0, int(hex_color[0:2], 16) * (1 - factor))
    g = max(0, int(hex_color[2:4], 16) * (1 - factor))
    b = max(0, int(hex_color[4:6], 16) * (1 - factor))
    return f"#{int(r):02x}{int(g):02x}{int(b):02x}"


def _hex_to_rgb(hex_color: str) -> tuple:
    """#RRGGBB -> (r,g,b) 0-255"""
    h = hex_color.lstrip("#")
    if len(h) != 6:
        return (255, 107, 107)
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


def _find_chinese_font() -> str:
    """查找支持中文的系统字体路径。load_default() 不支持中文会显示方框，必须用 TrueType。"""
    # 1. 本 skill 目录下的 fonts/（可放置 NotoSansSC-Regular.otf 等）
    script_dir = Path(__file__).resolve().parent
    skill_dir = script_dir.parent
    for name in ("NotoSansCJKsc-Regular.otf", "NotoSansSC-Regular.otf", "SourceHanSansSC-Regular.otf",
                 "PingFang.ttc", "msyh.ttc", "simhei.ttf"):
        p = skill_dir / "fonts" / name
        if p.exists():
            return str(p)

    # 2. macOS：仅用直接路径，沙箱下 subprocess 可能卡住
    mac_fonts = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ]
    for p in mac_fonts:
        if Path(p).exists():
            return p

    # 3. Linux: fontconfig（仅非 macOS，沙箱下 subprocess 可能卡住）
    if sys.platform != "darwin":
        try:
            import subprocess
            r = subprocess.run(
                ["fc-match", "-f", "%{file}", "zh", "sans"],
                capture_output=True, text=True, timeout=2,
            )
            if r.returncode == 0 and r.stdout.strip() and Path(r.stdout.strip()).exists():
                return r.stdout.strip()
        except Exception:
            pass

    # 4. Linux 常见路径
    linux_fonts = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    ]
    for p in linux_fonts:
        if Path(p).exists():
            return p

    # 5. Windows
    win_fonts = [
        os.path.expandvars(r"%WINDIR%\Fonts\msyh.ttc"),
        os.path.expandvars(r"%WINDIR%\Fonts\simhei.ttf"),
    ]
    for p in win_fonts:
        if p and Path(p).exists():
            return p

    raise RuntimeError(
        "未找到中文字体，封面标题会显示为方框。请任选其一：\n"
        "  - macOS/Linux: 已有 PingFang/Noto 即可\n"
        "  - Linux: apt install fonts-noto-cjk\n"
        "  - 或在 .skills/xiaohongshu-writer/fonts/ 放入 NotoSansCJKsc-Regular.otf"
    )


def _wrap_text(text: str, chars_per_line: int = 16) -> list:
    """将长文本按每行字数拆分为多行。避免在英文单词中间断行。"""
    import re
    text = re.sub(r"\s+", " ", text.strip())
    text = re.sub(r"[\U0001F300-\U0001F9FF]", "", text)
    if not text:
        return []
    lines = []
    i = 0
    while i < len(text) and len(lines) < 6:
        end = min(i + chars_per_line, len(text))
        chunk = text[i:end]
        # 避免在英文单词中间断开：若 chunk 以字母结尾且下一字符也是字母，则向后延伸至单词结束
        if end < len(text) and re.search(r"[a-zA-Z0-9]$", chunk):
            j = end
            while j < len(text) and re.search(r"[a-zA-Z0-9]", text[j]):
                j += 1
            chunk = text[i:j]
            end = j
        if chunk.strip():
            lines.append(chunk)
        i = end
    return lines


def _render_with_pillow(
    title: str,
    body: str,
    hashtags: list,
    accent_color: str,
    style: str,
    w: int = 1024,
    h: int = 1536,
) -> bytes:
    """用 Pillow 绘制图文封面：标题 + 正文摘要 + 标签。高质量图文风格。"""
    # Level 2 沙箱下缩小尺寸以加快生成、避免 120s 超时
    if os.environ.get("SKILLBOX_SANDBOX") == "1":
        w, h = 512, 768
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError:
        raise RuntimeError("需要安装 Pillow: pip install Pillow")

    img = Image.new("RGB", (w, h))
    r, g, b = _hex_to_rgb(accent_color)
    r2, g2, b2 = _hex_to_rgb(_darken_hex(accent_color, 0.5))
    # 用 1×h 渐变条 + resize 替代 1536 次 draw.line，显著提速（原实现可导致 120s 超时）
    grad = Image.new("RGB", (1, h))
    pix = grad.load()
    for y in range(h):
        t = y / h
        pix[0, y] = (
            int(r * (1 - t) + r2 * t),
            int(g * (1 - t) + g2 * t),
            int(b * (1 - t) + b2 * t),
        )
    grad = grad.resize((w, h), Image.NEAREST)
    img.paste(grad, (0, 0))
    draw = ImageDraw.Draw(img)

    font_path = _find_chinese_font()
    pad_x, pad_y = 80, 60

    # 1. 主标题（上方，大字）
    title_font_size = min(72, max(48, w // max(1, min(20, len(title))) * 2))
    title_font = ImageFont.truetype(font_path, title_font_size)
    title_lines = _wrap_text(title, 12) or [title[:12]]
    title_block = "\n".join(title_lines[:2])
    if hasattr(draw, "textbbox"):
        tb = draw.textbbox((0, 0), title_block, font=title_font)
        tw, th = tb[2] - tb[0], tb[3] - tb[1]
    else:
        tw, th = draw.textsize(title_block, font=title_font)
    tx = (w - tw) // 2
    ty = int(h * 0.15)
    draw.text((tx, ty), title_block, fill=(255, 255, 255), font=title_font)

    # 2. 正文摘要（中部，多行）
    body_lines = _wrap_text(body, 18)
    if body_lines:
        body_font_size = 36
        body_font = ImageFont.truetype(font_path, body_font_size)
        line_height = int(body_font_size * 1.5)
        by = int(h * 0.38)
        for i, line in enumerate(body_lines[:5]):
            if hasattr(draw, "textbbox"):
                bbox = draw.textbbox((0, 0), line, font=body_font)
                bw = bbox[2] - bbox[0]
            else:
                bw, _ = draw.textsize(line, font=body_font)
            bx = (w - bw) // 2
            draw.text((bx, by + i * line_height), line, fill=(255, 255, 255), font=body_font)

    # 3. 标签（底部）
    if hashtags:
        tag_str = "  ".join(hashtags[:5])
        tag_font_size = 28
        tag_font = ImageFont.truetype(font_path, tag_font_size)
        if hasattr(draw, "textbbox"):
            tagb = draw.textbbox((0, 0), tag_str, font=tag_font)
            tagw, tagh = tagb[2] - tagb[0], tagb[3] - tagb[1]
        else:
            tagw, tagh = draw.textsize(tag_str, font=tag_font)
        tagx = (w - tagw) // 2
        tagy = int(h * 0.88)
        # 深色底条增强可读性
        draw.rectangle(
            [(tagx - 24, tagy - 10), (tagx + tagw + 24, tagy + tagh + 14)],
            fill=(40, 40, 50),
            outline=None,
        )
        draw.text((tagx, tagy), tag_str, fill=(255, 255, 255), font=tag_font)

    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return buf.getvalue()


def _screenshot_html(html_content: str, width: int = 1024, height: int = 1536) -> bytes:
    """用 Playwright 渲染 HTML 并截图，返回 PNG 字节。"""
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        raise RuntimeError("需要安装 playwright: pip install playwright")

    with tempfile.NamedTemporaryFile(suffix=".html", delete=False) as f:
        f.write(html_content.encode("utf-8"))
        file_path = f.name

    try:
        with sync_playwright() as p:
            browser = p.chromium.launch()
            page = browser.new_page(viewport={"width": width, "height": height})
            page.goto(Path(file_path).as_uri())
            page.wait_for_load_state("networkidle")
            screenshot_bytes = page.screenshot(type="png")
            browser.close()
        return screenshot_bytes
    finally:
        Path(file_path).unlink(missing_ok=True)


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        print(json.dumps({"success": False, "error": f"无效 JSON 输入: {e}"}, ensure_ascii=False))
        sys.exit(1)

    content = input_data.get("content")
    if not content or not isinstance(content, dict):
        print(json.dumps({
            "success": False,
            "error": "缺少 content 参数。请传入 Agent 生成的内容：{title, body, hashtags, thumbnail: {cover_title?, accent_color?, style?}}",
        }, ensure_ascii=False))
        sys.exit(1)

    generate_thumbnail = input_data.get("generate_thumbnail", True)
    title = content.get("title") or ""
    thumb = content.get("thumbnail") or {}
    cover_title = thumb.get("cover_title") or title or "小红书笔记"
    accent_color = _norm_color(thumb.get("accent_color") or "#FF6B6B")
    style = thumb.get("style") or "gradient"

    if not cover_title:
        cover_title = "小红书笔记"

    # 缩略图：仅用 Pillow（快速，2–5 秒）。Playwright 首次启动需下载 Chromium，易超时，不再作为回退。
    if generate_thumbnail:
        img_bytes = None
        source = None
        body_text = content.get("body") or ""
        tags = content.get("hashtags") or []
        try:
            img_bytes = _render_with_pillow(cover_title, body_text, tags, accent_color, style)
            source = "pillow"
        except Exception as e1:
            thumb["image_error"] = (
                f"Pillow 生成失败: {e1}。"
                "请确保已安装 Pillow 及中文字体（macOS 自带 PingFang；Linux: apt install fonts-noto-cjk；"
                "或于 .skills/xiaohongshu-writer/fonts/ 放入 NotoSansCJKsc-Regular.otf）。"
            )
        if img_bytes:
            thumb["image_source"] = source
            # 同时保存到文件，便于用户查看
            try:
                skill_dir = os.environ.get("SKILL_DIR", "")
                out_dir = Path(skill_dir).parent.parent if skill_dir else Path.cwd()
                out_path = Path(out_dir) / "xiaohongshu_thumbnail.png"
                out_path.write_bytes(img_bytes)
                thumb["image_path"] = str(out_path)
            except Exception:
                pass
            # 有 image_path 时不输出 base64，避免 stdout 过大（>64KB）导致 skillbox 管道死锁
            if "image_path" not in thumb:
                thumb["image_base64"] = base64.b64encode(img_bytes).decode("ascii")

    content["thumbnail"] = thumb

    print(json.dumps({
        "success": True,
        "title": content.get("title"),
        "body": content.get("body"),
        "hashtags": content.get("hashtags", []),
        "thumbnail": content.get("thumbnail", thumb),
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
