# 贡献指南

感谢你对 SkillLite 项目的关注！

## 贡献方式

- **Bug 报告**：提交 issue 并附上详细的复现步骤
- **功能建议**：提交 issue 进行讨论
- **代码贡献**：提交 Pull Request
- **文档改进**：修复错别字、改进文档、添加示例
- **Skills 分享**：创建并分享新的 Skills

## 开发环境设置

### 前置要求

- **Rust**（最新稳定版）- 用于沙箱执行器
- **Python 3.8+** - 用于 SDK
- **macOS 或 Linux** - 暂不支持 Windows

### 设置步骤

```bash
# 克隆仓库
git clone https://github.com/EXboys/skilllite.git
cd skilllite

# 构建 Rust 沙箱
cd skilllite
cargo build --release
cargo install --path .

# 设置 Python SDK
cd ../python-sdk
pip install -e ".[dev]"

# 配置环境
cp .env.example .env
```

## Pull Request 流程

1. Fork 仓库
2. 从 `main` 创建分支：`git checkout -b feature/your-feature`
3. 提交清晰的 commit
4. 测试：`cd skilllite && cargo test` 和 `cd python-sdk && pytest`
5. 提交 PR 并附上清晰的描述

### 指南

- 保持 PR 专注且大小适中
- 为新功能编写测试
- 确保 CI 通过后再请求 review

## 代码风格

### Rust
- 遵循标准 Rust 规范
- 运行 `cargo fmt` 和 `cargo clippy`

### Python
- 遵循 PEP 8
- 使用类型提示
- 使用 `black` 和 `isort` 格式化

## 文档规范

- **代码注释**：所有代码注释和 docstring 使用英文
- **用户文档**：同时维护英文和中文版本
- **README 同步**：同时更新 `README.md` 和 `README_CN.md`

## 安全问题

发现安全漏洞时，**请勿**公开提交 issue。请联系：security@skilllite.dev

## 许可证

通过贡献代码，你同意你的贡献将在 MIT 许可证下发布。

## 项目结构

```
skillLite/
├── skilllite/          # Rust 沙箱执行器
├── python-sdk/         # Python SDK
├── .skills/            # 内置 Skills（示例）
├── benchmark/          # 性能测试
├── test/               # LangChain 集成测试（run_tests.py，.gitignore）
├── tests/              # Pytest 单元测试（test_core、test_mcp 等）
└── docs/               # 文档
    ├── en/             # 英文文档
    └── zh/             # 中文文档
```

### 测试目录说明

| 目录 | 用途 | 运行命令 |
|------|------|----------|
| `test/` | LangChain/SkillLite 集成测试，需 skills | `cd test && python run_tests.py` |
| `tests/` | 核心/SDK 的 Pytest 单元测试 | `cd python-sdk && pytest` 或 `pytest tests/` |

## 行为准则

我们承诺为每个人提供无骚扰的参与体验。

**积极行为：**
- 展现同理心和善意
- 尊重不同的观点和经验
- 给予和接受建设性反馈

**不可接受的行为：**
- 骚扰、挑衅或人身攻击
- 未经许可发布他人私人信息

问题报告：security@skilllite.dev

---

*改编自 [Contributor Covenant](https://www.contributor-covenant.org) v2.1*

