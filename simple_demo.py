#!/usr/bin/env python3
"""
SkillLite 示例 - 使用内置的增强功能

优化说明：
- 使用 SDK 内置的智能任务完成检测
- 使用 SDK 内置的任务执行指导
- 代码量从 ~600 行减少到 ~30 行

使用方法:
    1. cp .env.example .env
    2. 编辑 .env 填入你的配置
    3. python3 simple_demo.py
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'skilllite-sdk'))

from skilllite import SkillRunner


def interactive_confirmation(report: str, scan_id: str) -> bool:
    """交互式确认回调 - 当检测到高危操作时提示用户确认"""
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

    # 创建 Runner（自动加载 .env 配置）
    # 内置功能：
    # ✅ 智能任务完成检测
    # ✅ 任务执行指导 system prompt
    # ✅ 自动规划和迭代
    # ✅ 安全确认回调（sandbox_level=3 时生效）
    runner = SkillRunner(
        verbose=True,           # 显示详细日志
        max_iterations=30,      # 最多 30 次迭代
        confirmation_callback=interactive_confirmation  # 安全确认回调
    )
    
    print(f"📡 API: {runner.base_url}")
    print(f"🤖 模型: {runner.model}")
    print(f"📦 已加载 Skills: {runner.manager.skill_names()}")
    print()
    
    # ============================================================
    # 👇 在这里修改你要测试的用户消息 👇
    # ============================================================
    
    # user_message = "帮我创建一个简单的数据分析技能"
    user_message = "深圳今天天气怎样，适合除去玩吗？" 

    # user_message = "帮忙写一首关于skilllite的诗歌"
    
    # ============================================================
    # 👆 在这里修改你要测试的用户消息 👆
    # ============================================================
    
    # 一行代码运行！所有复杂逻辑都内置在 SDK 中
    result = runner.run(user_message)
    
    print()
    print("=" * 60)
    print(f"🤖 最终结果: {result}")
    print("=" * 60)
