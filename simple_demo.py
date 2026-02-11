#!/usr/bin/env python3
"""
SkillLite demo - built-in enhanced features

- SDK built-in task completion detection
- SDK built-in task execution guidance
- Reduced from ~600 lines to ~30 lines

Usage:
    1. cp .env.example .env
    2. Edit .env with your config
    3. skilllite init   # optional, pre-install Skill deps (e.g. Pillow)
    4. python3 simple_demo.py
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'skilllite-sdk'))

from skilllite import SkillRunner


def interactive_confirmation(report: str, scan_id: str) -> bool:
    """Interactive confirmation callback - prompts user when high-risk ops detected"""
    print("\n" + "=" * 60)
    print(report)
    print("=" * 60)
    while True:
        response = input("⚠️  是否允许执行？(y/n): ").strip().lower()
        if response in ['y', 'yes', '是']:
            return True
        elif response in ['n', 'no', '否']:
            return False
        print("请输入 'y' 或 'n'")


if __name__ == "__main__":
    print("=" * 60)
    print("🚀 SkillLite 示例（使用内置增强功能）")
    print("=" * 60)
    print()

    # Create Runner (auto-loads .env)
    # Built-in: task completion detection, task guidance, planning, confirmation callback
    runner = SkillRunner(
        verbose=True,            # verbose logs
        max_iterations=50,       # max iterations
        execution_timeout=300,   # xiaohongshu-writer may install Pillow/Playwright on first run
        confirmation_callback=interactive_confirmation,
    )
    
    print(f"📡 API: {runner.base_url}")
    print(f"🤖 模型: {runner.model}")
    print(f"📦 已加载 Skills: {runner.manager.skill_names()}")
    print()
    
    # ============================================================
    # 👇 Edit user message to test here 👇
    # ============================================================
    
    # user_message = "帮我创建一个简单的数据分析技能"
    # user_message = "深圳今天天气怎样，适合除去玩吗？" 

    # user_message = "分析一下这组数据：[[1,2],[3,4]]，列名是 a 和 b，计算相关系数"

    # user_message = "帮忙写一首关于skilllite的诗歌"
    
    user_message = "写一个关于本项目推广的小红书的图文，使用小红书的skills"
    # ============================================================
    # 👆 Edit user message to test above 👆
    # ============================================================
    
    # Single line to run - all logic built into SDK
    result = runner.run(user_message)
    
    print()
    print("=" * 60)
    print(f"🤖 最终结果: {result}")
    print("=" * 60)
