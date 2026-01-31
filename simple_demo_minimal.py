#!/usr/bin/env python3
"""
最极简版 SkillLite 示例 - 一行代码运行 Skill

使用方法:
    1. cp .env.example .env
    2. 编辑 .env 填入你的配置
    3. python3 simple_demo_minimal.py
"""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'skilllite'))

from skilllite import quick_run

# 一行代码搞定！
# result = quick_run("请帮我把以下文本进行处理，全部变成大写：Hello World!", verbose=True)

result = quick_run("写一篇赞美的 SkillLite 的诗歌", verbose=True)
print(f"\n最终结果: {result}")
