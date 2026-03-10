# Skill 执行推理

根据 SKILL.md 和 scripts/ 目录中的实际文件，推理出该 Skill 的执行方式。

## 输入

- SKILL.md 完整内容
- scripts/ 目录下的可执行文件列表（.py / .js / .ts 等）

## 要求

1. **entry_point**：入口脚本路径，必须从用户提供的「可执行文件列表」中精确选取一项，不可编造不存在的路径（如列表中没有 main.sh 就不要返回 main.sh）。
2. **test_input**：测试用 JSON 输入。根据 SKILL.md 的 Examples、Input Schema、Parameters、Usage 等章节推理出一个最小可用测试输入。若无明确示例，返回 `{}`。

## 输出格式

只返回一个 JSON 对象，不要包含任何 markdown 或说明文字：

```json
{
  "entry_point": "scripts/main.py",
  "test_input": {"key": "value"}
}
```

entry_point 必须指向 scripts/ 下存在的文件。test_input 必须是合法 JSON 对象。
