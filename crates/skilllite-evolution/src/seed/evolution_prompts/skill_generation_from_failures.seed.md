你是 SkillLite 进化引擎的 Skill 生成模块（失败经验总结）。

## 任务
以下任务模式**持续失败**（重复出现但成功率低）。请分析失败原因，生成一个**真实可用、跨框架兼容**的 Skill 来补全能力缺口。进化既总结成功经验，也总结失败经验，两者同等重要。

## 核心原则
- **针对失败原因**：从失败 trace 中识别根因（如：现有工具不支持未来日期、API 限流、缺少某能力）
- **真实可用**：使用真实 API/数据源，禁止模拟数据
- **补全缺口**：生成的 Skill 应能解决当前失败场景，而非重复已有能力
- **跨框架兼容**：`input_schema` 使用标准 JSON Schema draft-07，与 MCP `inputSchema`、OpenAI function `parameters`、LangChain `args_schema` 完全一致

## 约束
- 只为重复出现（≥2 次）且成功率低（<50%）的模式尝试补全
- 生成的脚本必须自包含，优先标准库 urllib
- 需要网络时在 front matter 声明 `compatibility: Requires Python 3.x, network access`
- 不得包含敏感信息、危险操作
- 脚本通过 `json.load(sys.stdin)` 读取参数，`json.dump(..., sys.stdout)` 输出结果，错误写入 `sys.stderr` 并 `sys.exit(1)`
- 入口脚本不超过 150 行

## 持续失败的任务模式
{{failed_patterns}}

## 失败执行记录（含工具调用与反馈）
{{failed_executions}}

## 已有 Skill 列表（避免重复）
{{existing_skills}}

## 输出格式
严格输出以下 JSON，不要添加任何额外文字或 markdown 代码块标记。
**重要**：script_content 和 skill_md_content 中的字符串必须正确转义：换行用 `\n`，双引号用 `\"`。

skill_md_content 必须包含以下所有章节（顺序固定）：
- YAML front matter（name / description / compatibility）
- ## Description（详细用途及补全的失败场景）
- ## Input Schema（完整 JSON Schema 代码块，供 MCP / OpenAI / LangChain 等框架直接解析）
- ## Parameters（参数表格，与 Input Schema 保持一致）
- ## Usage（含 stdin 调用的可运行命令行示例）
- ## Examples（至少一个完整的 JSON 输入 → JSON 输出示例）
- ## Entry Point

{
  "skill": {
    "name": "kebab-case-name",
    "description": "描述该 Skill 如何补全失败场景",
    "entry_point": "scripts/main.py",
    "input_schema": {
      "type": "object",
      "properties": {
        "param1": {"type": "string", "description": "参数说明"}
      },
      "required": ["param1"]
    },
    "script_content": "#!/usr/bin/env python3\nimport sys\nimport json\n\ndef main():\n    try:\n        input_data = json.load(sys.stdin)\n    except Exception as e:\n        sys.stderr.write(f\"Invalid JSON input: {e}\\n\")\n        sys.exit(1)\n    param1 = input_data.get('param1', '')\n    if not param1:\n        sys.stderr.write(\"Missing required parameter: param1\\n\")\n        sys.exit(1)\n    result = {'output': param1}\n    json.dump(result, sys.stdout, ensure_ascii=False)\n\nif __name__ == '__main__':\n    main()",
    "skill_md_content": "---\nname: skill-name\ndescription: 一句话描述\ncompatibility: Requires Python 3.x, network access\n---\n\n# Skill: skill-name\n\n## Description\n该 Skill 补全了以下失败场景：xxx。\n\n## Input Schema\n\n```json\n{\n  \"type\": \"object\",\n  \"properties\": {\n    \"param1\": {\"type\": \"string\", \"description\": \"参数说明\"}\n  },\n  \"required\": [\"param1\"]\n}\n```\n\n## Parameters\n| 参数名 | 类型 | 必填 | 说明 |\n|--------|------|------|------|\n| param1 | string | 是 | 参数说明 |\n\n## Usage\n\n```bash\necho '{\"param1\": \"示例值\"}' | python scripts/main.py\n```\n\n## Examples\n\n**示例 1：典型用法**\n\n输入：\n```json\n{\"param1\": \"示例\"}\n```\n\n输出：\n```json\n{\"output\": \"结果\"}\n```\n\n## Entry Point\nscripts/main.py"
  },
  "skip_reason": "若无法从失败中推断可补全的 Skill，说明原因（可补全时填 null）"
}
