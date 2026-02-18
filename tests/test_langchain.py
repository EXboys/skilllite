"""
LangChain + SkillLite integration demo.

Requires: pip install langchain-skilllite langchain-openai
"""
import os
from dotenv import load_dotenv
from langchain_skilllite import SkillLiteToolkit
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import create_react_agent

# 加载环境变量
load_dotenv()

# Uses langchain-skilllite (SkillLiteToolkit)
def confirm(report: str, scan_id: str) -> bool:
    print(report)
    return input("Continue? [y/N]: ").lower() == 'y'

tools = SkillLiteToolkit.from_directory(
    "./.skills",
    sandbox_level=3,
    confirmation_callback=confirm,
)

# 使用环境变量配置 LLM
llm = ChatOpenAI(
    base_url=os.getenv("BASE_URL", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    api_key=os.getenv("API_KEY"),
    model=os.getenv("MODEL", "qwen3-max"),
)

# 创建自定义 prompt
react_prompt = """你是一个工具执行助手，可以使用以下工具来帮助用户完成任务。

当用户要求你使用某个工具时，你必须调用该工具执行。请逐步思考，合理使用工具。

当你有最终答案时，直接回复用户。"""

# 创建 agent
agent = create_react_agent(llm, tools, prompt=react_prompt)

# 测试 agent
import asyncio

async def test_agent():
    # 测试 dangerous-test skill，它有系统命令执行代码，应该触发安全确认
    response = await agent.ainvoke({
        "messages": [("user", "使用 dangerous-test 技能执行命令 'echo hello'")]
    })
    print(response)

asyncio.run(test_agent())

