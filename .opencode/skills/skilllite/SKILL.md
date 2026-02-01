---
name: skilllite
description: 在安全沙箱中执行代码或预定义技能。当需要运行不信任的代码、网络请求、数据处理时使用。
---

## 概述

SkillLite 提供了一个安全的沙箱执行环境。代码在系统级沙箱（macOS Seatbelt / Linux Namespace）中隔离运行，防止恶意代码影响主机系统。

## 何时使用 SkillLite 而不是 bash

| 场景 | 用 bash | 用 SkillLite |
|-----|---------|-------------|
| git 操作 | ✅ | |
| 读取项目文件 | ✅ | |
| 执行用户提供的代码 | | ✅ |
| 网络请求/API 调用 | | ✅ |
| 数据分析处理 | | ✅ |
| 运行不信任的脚本 | | ✅ |
| 执行可能危险的命令 | | ✅ |

## 可用工具

### 1. skilllite_execute_code
在沙箱中执行任意代码（Python/JavaScript/Bash）。

**参数：**
- `language`: "python" | "javascript" | "bash"
- `code`: 要执行的代码
- `confirmed`: 是否确认执行（高危代码需要）
- `scan_id`: 扫描 ID（确认执行时需要）

**安全确认流程：**
当检测到危险代码时，会返回安全报告和 `scan_id`。向用户展示安全问题后，如果用户同意执行，需要再次调用时设置 `confirmed=true` 和返回的 `scan_id`。

### 2. skilllite_run_skill
执行预定义技能。

**参数：**
- `skill_name`: 技能名称
- `input`: 技能的输入参数（JSON 对象）

### 3. skilllite_list_skills
查看所有可用的预定义技能。无需参数。

### 4. skilllite_get_skill_info
获取指定技能的详细信息，包括输入参数模式。

**参数：**
- `skill_name`: 技能名称

### 5. skilllite_scan_code
仅扫描代码安全性，不执行。用于预检查代码是否安全。

**参数：**
- `language`: "python" | "javascript" | "bash"
- `code`: 要扫描的代码

## 预定义技能

- **calculator**: A simple calculator that can add, subtract, multiply, and divide numbers. Use when the user needs to perform basic arithmetic operations.
- **text-processor**: 文本处理工具。支持的操作(operation)：uppercase(大写)、lowercase(小写)、reverse(反转)、trim(去空白)、count(统计)。参数：text(文本)、operation(操作类型)。
- **data-analyzer**: 数据分析工具，使用 pandas 进行数据统计分析。当用户需要对数据进行统计分析、计算均值、求和、描述性统计时使用。
- **nodejs-test**: A simple Node.js skill for testing
- **skill-creator**: Create or update AgentSkills. Use when designing, structuring, or packaging skills with scripts, references, and assets.
- **http-request**: 发起 HTTP 网络请求，支持 GET、POST、PUT、DELETE、PATCH 方法。当用户需要调用 API、获取网页内容、发送数据到服务器时使用。
- **data-analysis**: 提供全面的数据清洗和统计分析功能。支持处理缺失值、去除重复值、数据类型转换、异常值处理、字符串清理等数据清洗操作，以及描述性统计、相关性分析、分组统计、假设检验、数据分布分析等统计功能。当用户需要对数据进行清洗预处理或统计分析时使用。
- **weather**: 查询城市天气信息。当用户询问某个城市的天气、温度、湿度等信息时使用。
- **writing-helper**: 高质量写作助手，专注于创作自然、有温度、低AI感的文章内容


## 使用示例

### 执行 Python 代码
```
skilllite_execute_code(language="python", code="print(sum(range(1, 101)))")
```

### 处理危险代码
1. 调用 `skilllite_execute_code` 执行代码
2. 如果返回 `requires_confirmation=true`，向用户展示安全问题
3. 用户确认后，再次调用时带上 `confirmed=true` 和 `scan_id`

### 使用预定义技能
```
skilllite_list_skills()  # 查看可用技能
skilllite_get_skill_info(skill_name="calculator")  # 查看技能参数
skilllite_run_skill(skill_name="calculator", input={"operation": "add", "a": 5, "b": 3})
```
