# SkillLite Assistant

Tauri 2 + React 18 + TypeScript + Vite 桌面应用脚手架。

**所有 npm 命令必须在当前目录（`crates/skilllite-assistant`）下执行**，仓库根目录没有 `package.json`。

## 开发

```bash
cd crates/skilllite-assistant   # 若在仓库根目录，先执行这句
npm install
npm run tauri dev
```

## 构建

```bash
cd crates/skilllite-assistant   # 若在仓库根目录，先执行这句
npm run tauri:build
# 或：npm run tauri build（DMG 可能需更长时间）
```

构建时会自动执行 `scripts/prebuild-skilllite.sh`，安装完整版 skilllite（含 `memory_vector`）到 `~/.skilllite/bin/`：

- `mkdir -p ~/.skilllite/bin`
- `rm -f ~/.skilllite/bin/skilllite`
- `cargo install --path skilllite --features memory_vector --root ~/.skilllite`

桌面应用通过 `skilllite agent-rpc` 子进程调用，需确保 `~/.skilllite/bin` 在 PATH 中（如 `export PATH="$HOME/.skilllite/bin:$PATH"`）。

如需单独预装 skilllite：`npm run prebuild:tauri`

## 环境与 Skills

- **API Key**：在项目根目录或 workspace 的 `.env` 中设置 `OPENAI_API_KEY`
- **Skills**：会自动从 workspace 向上查找 `.skills` 或 `skills` 目录
- **skilllite**：需已安装（`pip install skilllite` 或 `cargo install --path skilllite`）
- **Level 3 确认**：Skill 执行前会弹出安全扫描确认弹窗，点击「允许」继续执行
- **IDE 三栏**：顶栏 **IDE** 或 **设置 → 工作区与沙箱 → IDE 三栏主界面** 可切换为「工作区文件树 | 编辑器 | 对话」；**左右两条竖向分隔线可拖拽**调宽（宽度会记住）。左侧 **会话** 标签仍打开原会话列表。大目录会跳过 `node_modules`、`target` 等；敏感路径与写入规则一致（如 `.env` 不可读/写）。关闭 IDE 布局后恢复右侧状态栏。**中间栏**：Markdown（`.md` / `.mdx` / `.mdc` 等）默认 **预览**，可切 **编辑** 并保存；常见 **图片**（如 png / jpg / webp / svg）与 **视频**（如 mp4 / webm / mov）为 **仅预览**（内嵌播放，不在此写回二进制）。图片/视频依赖 Tauri **`assetProtocol`**（见 `src-tauri/tauri.conf.json`）；修改该配置后需 **重启** `tauri dev` 或重新打包应用。若仍无法显示，请确认文件路径落在 `assetProtocol.scope.allow` 所允许的前缀下（默认可访问用户主目录、文档、桌面、下载、临时目录等）。聊天里 **`read_file` 工具结果**默认以约 **5 行可滚动**预览展示；**点击预览**与 **「在 IDE 中打开」** 相同（有路径时开启 IDE 并在中间栏打开该文件；无路径时打开「全屏查看 / 编辑」）。**`list_directory`** 结果为同样高度的可滚动树预览。
- **聊天**：任务计划、工具调用、**工具回复**与「执行确认 / 需要你的确认」均收在可折叠的 **内部步骤** 时间线中（**待操作**、本轮加载中会默认展开；**仅当前会话里「最后一段」内部步骤**在含 **`read_file` / `list_directory` 成功结果**时默认展开，更早的步骤仍折叠，避免长会话刷屏；收起时显示 **待操作** 标签）；可在 **设置 → Agent 预算** 或输入框下方开启 **自动允许执行确认**（仅自动点「允许」，不代替澄清选项）
- **图片（视觉模型）**：输入框旁 **图片** 使用 **系统原生文件选择框**（Tauri 桌面端；避免 WebView 拦截隐藏的 `<input type="file">`），可选 PNG / JPEG / WebP / GIF（单张 ≤5MB，每轮最多 6 张），选后会在输入区上方显示缩略图预览。随消息发给已配置的 **支持视觉** 的模型（如 GPT‑4o、Claude 3.5 等）。历史会写入 transcript 并可在重开会话后预览；**MiniMax Coding Plan** 路径不支持附图。
- **自进化 · 变更对比**：右侧 **自进化 → 详情与审核 → 变更对比** 中，对已写入 changelog 的 prompts 文件（如 `rules.json`）可用 **左/右版本下拉** 任选两版：`当前（prompts 文件）` 与各 **`prompts/_versions/<txn_id>/` 快照**（按快照 mtime 新→旧）。可切换 **对比** / **两侧原文**；旧快照可能因 `SKILLLITE_EVOLUTION_SNAPSHOT_KEEP` 被清理。**最近进化事件**里类型为 **进化运行** 且带有 **txn** 的行可 **点击**，会切到「变更对比」并把左侧默认设为该 txn；备注里含 memory knowledge 时另有 **打开记忆详情** 链到记忆窗口（具体分片内容仍须在列表中打开）。**尚无 changelog 对比时**，同页可展开 **手动编辑 prompts**：应用内读写 `chat/prompts` 白名单文件，或 **打开 prompts 目录** 用外部编辑器（路径在数据目录而非工程根，见界面说明）。

## 图标

项目已包含默认占位图标（`src-tauri/icons/`）。更换应用图标：

1. 准备 512×512 或 1024×1024 的方形 PNG
2. 替换 `app-icon.png` 或指定路径
3. 运行 `npm run icon` 重新生成
