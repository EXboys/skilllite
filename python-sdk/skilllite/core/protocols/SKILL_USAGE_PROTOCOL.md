# Skill Usage Protocol (强制协议)

> **此协议是所有对外接口的强制要求。所有适配器（LangChain、LlamaIndex、MCP 以及任何未来的集成）都必须遵循此协议。违反此协议视为 BUG。**

## 核心原则

### 两阶段使用协议

Skill 的使用分为两个阶段：

#### Phase 1 - 选择阶段 (Selection Phase)

LLM 看到所有可用工具的列表，通过 `name` 和 `description`（短描述）来决定使用哪个工具。

- 此阶段**只需要** `name` 和 `description`（来自 SKILL.md YAML front matter）
- 此阶段**不需要**完整文档
- 用途：让 LLM 判断该用哪个工具

对应各适配器：
- **LangChain**: 工具注册时 LLM 通过 tool name + description 选择
- **LlamaIndex**: 同上
- **MCP**: `list_skills` 返回 name + description 列表

#### Phase 2 - 使用阶段 (Usage Phase)

LLM 决定使用某个工具后，**必须加载该 skill 的完整 SKILL.md 文档**。

- 完整文档包含：Usage 说明、Examples 示例、参数格式等
- LLM 根据文档中的示例**自行推断**正确的参数名和格式
- **不依赖** `args_schema` 或 `input_schema` 来告诉 LLM 参数格式

对应各适配器：
- **LangChain**: `to_tools()` 中 `description` 使用 `skill.get_full_content()`
- **LlamaIndex**: `to_tools()` 中 `description` 使用 `skill.get_full_content()`
- **MCP**: `get_skill_info` 返回 `skill.get_full_content()`

---

## 禁止事项

### 1. 永远不要修改 SKILL.md

SKILL.md 是用户创建的内容，**任何情况下都不能修改**。原因：
- 破坏用户自定义内容
- 无法保证兼容性
- 违反 skill 协议的基本原则

### 2. 永远不要在 SKILL.md 中添加 runtime/input_schema

不要为了让 LLM 理解参数格式而在 SKILL.md 中添加 `input_schema`、`args_schema` 或 `## Runtime` 等内容。

### 3. 永远不要用 args_schema 替代完整文档

不要试图从 SKILL.md 中提取 `input_schema` 来生成 Pydantic `args_schema`。参数推断是 LLM 的工作，不是代码的工作。

---

## 实现参考

### LangChain 适配器 (`adapters/langchain.py`)

```python
def to_tools(self):
    for skill in self.get_executable_skills():
        # Phase 2: 使用完整 SKILL.md 内容
        full_content = skill.get_full_content()
        tool_description = full_content or skill.description or f"Execute the {skill.name} skill"
        tool = SkillLiteTool(
            name=skill.name,
            description=tool_description,
            ...
        )
```

### LlamaIndex 适配器 (`adapters/llamaindex.py`)

```python
def to_tools(self):
    for skill in self.get_executable_skills():
        # Phase 2: 使用完整 SKILL.md 内容
        full_content = skill.get_full_content()
        tool_description = full_content or skill.description or f"Execute the {skill.name} skill"
        tool = FunctionTool.from_defaults(
            fn=fn,
            name=skill.name,
            description=tool_description
        )
```

### MCP 适配器 (`mcp/server.py`)

```
Phase 1: list_skills → 返回 name + description（短描述）
Phase 2: get_skill_info → 返回 skill.get_full_content()（完整 SKILL.md）
Phase 3: run_skill → 执行
```

---

## 新增适配器检查清单

新增任何框架适配器时，必须确认以下几点：

- [ ] Selection Phase 只暴露 `name` 和 `description`
- [ ] Usage Phase 加载完整 `SKILL.md` 内容（`skill.get_full_content()`）
- [ ] 不修改任何 SKILL.md 文件
- [ ] 不添加 `args_schema` 或 `input_schema` 到工具定义
- [ ] LLM 通过文档示例推断参数，不依赖 schema

