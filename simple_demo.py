#!/usr/bin/env python3
"""
SkillLite demo — 通过 chat() API 调用，无需关心 binary 命令行

非交互模式 (--message) 下，高风险操作会自动通过，无确认提示。
交互确认请使用: skilllite chat

xiaohongshu-writer 需 Playwright，脚本会自动设置 SKILLBOX_ALLOW_PLAYWRIGHT=1。
若仍报 BlockingIOError，可在 .env 中显式添加 SKILLBOX_ALLOW_PLAYWRIGHT=1。

Usage:
    1. cp .env.example .env
    2. Edit .env with your config
    3. skilllite init   # optional, pre-install Skill deps (e.g. Pillow)
    4. python3 simple_demo.py
"""
import os
import sys

# Add python-sdk to path for skilllite package
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "python-sdk"))

# Load .env into os.environ (binary also loads it, but ensure we have it for cwd)
def _load_env():
    env_path = os.path.join(os.path.dirname(__file__), ".env")
    if os.path.exists(env_path):
        with open(env_path) as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, _, v = line.partition("=")
                    k, v = k.strip(), v.strip().strip('"').strip("'")
                    if k and k not in os.environ:
                        os.environ[k] = v


if __name__ == "__main__":
    _load_env()

    if not os.environ.get("OPENAI_API_KEY") and not os.environ.get("API_KEY"):
        print("Error: Set OPENAI_API_KEY or API_KEY in .env", file=sys.stderr)
        sys.exit(1)

    # xiaohongshu-writer 需要 Playwright 启动浏览器，沙箱下需显式允许
    os.environ.setdefault("SKILLBOX_ALLOW_PLAYWRIGHT", "1")

    from skilllite import chat

    print("=" * 60)
    print("🚀 SkillLite 示例（chat API）")
    print("=" * 60)
    print()

    # 👇 Edit user message to test here 👇
    user_message = "写一个关于本项目推广的小红书的图文，使用小红书的skills"
    # user_message = "帮我创建一个简单的数据分析技能"
    # user_message = "分析一下这组数据：[[1,2],[3,4]]，列名是 a 和 b，计算相关系数"

    print(f"📡 消息: {user_message[:50]}...")
    print()

    result = chat(
        user_message,
        skills_dir=".skills",
        max_iterations=50,
        verbose=True,
        stream=True,
        cwd=os.path.dirname(os.path.abspath(__file__)),
    )

    print()
    print("=" * 60)
    print("🤖 任务完成" if result["success"] else f"Exit code: {result['exit_code']}")
    print("=" * 60)
    sys.exit(result["exit_code"])
