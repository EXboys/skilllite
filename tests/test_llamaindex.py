import os
from dotenv import load_dotenv
from skilllite import SkillManager
from skilllite.core.adapters.llamaindex import SkillLiteToolSpec
from llama_index.llms.openai_like import OpenAILike

# 加载环境变量
load_dotenv()

manager = SkillManager(skills_dir="./.skills")

# With security confirmation
def confirm(report: str, scan_id: str) -> bool:
    print(report)
    return input("Continue? [y/N]: ").lower() == 'y'

tool_spec = SkillLiteToolSpec.from_manager(
    manager,
    sandbox_level=3,
    confirmation_callback=confirm
)
tools = tool_spec.to_tool_list()

# Use with LlamaIndex agent
from llama_index.core.agent import ReActAgent

# 使用环境变量配置 LLM
llm = OpenAILike(
    api_base=os.getenv("BASE_URL", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    api_key=os.getenv("API_KEY"),
    model=os.getenv("MODEL", "qwen3-max"),
    is_chat_model=True,
)

# 创建自定义 prompt（可选）
react_prompt = """你是一个有帮助的AI助手，可以使用以下工具：
{tools_str}

请逐步思考，使用以下格式：
Thought: 你的推理过程
Action: 工具名称（必须是以下之一：[{tool_names}]）
Action Input: 工具需要的输入
Observation: 工具返回的结果

当你有最终答案时，使用：
Thought: 我已经知道答案
Answer: [你的最终答案]

开始！

Question: {input}
{scratchpad}"""

# 创建 agent
agent = ReActAgent(
    tools=tools,
    llm=llm,
    system_prompt=react_prompt,  # 使用自定义 prompt
    verbose=True,  # 显示思考过程
)

# 测试 agent
import asyncio

async def test_agent():
    # 测试 dangerous-test skill，它有系统命令执行代码，应该触发安全确认
    response = await agent.run("使用 dangerous-test 技能执行命令 'echo hello'")
    print(response)

asyncio.run(test_agent())