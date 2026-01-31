#!/usr/bin/env python3
"""
极简版 SkillLite 示例 - 使用封装后的 SkillRunner

对比 simple_demo.py，代码量从 ~150 行减少到 ~30 行！

使用方法:
    1. cp .env.example .env
    2. 编辑 .env 填入你的配置
    3. python3 simple_demo_v2.py
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'skilllite-sdk'))

from skilllite import SkillRunner

if __name__ == "__main__":
    # 创建 Runner（自动加载 .env 配置）
    runner = SkillRunner(verbose=True)
    
    print("=" * 60)
    print("🚀 SkillLite 极简示例")
    print("=" * 60)
    print(f"📡 API: {runner.base_url}")
    print(f"🤖 模型: {runner.model}")
    print(f"📦 已加载 Skills: {runner.manager.skill_names()}")
    print()
    
    # ============================================================
    # 👇 在这里修改你要测试的用户消息 👇
    # ============================================================
    
    user_message = "请帮我把以下文本进行处理，全部变成大写：  Hello,   World!  This is   a   test.   "
    
    # ============================================================
    # 👆 在这里修改你要测试的用户消息 👆
    # ============================================================
    
    # 一行代码运行！
    result = runner.run(user_message)
    
    print()
    print("=" * 60)
    print(f"🤖 最终结果: {result}")
    print("=" * 60)
