# SkillLite Assistant

Tauri 2 + React 18 + TypeScript + Vite 桌面应用脚手架。

## 开发

```bash
# 在 crates/skilllite-assistant 目录下
npm install
npm run tauri dev
```

## 构建

```bash
npm run tauri build
# 若 DMG 打包失败，可改用（跳过 AppleScript UI，避免超时）：
npm run tauri:build
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

## 图标

项目已包含默认占位图标（`src-tauri/icons/`）。更换应用图标：

1. 准备 512×512 或 1024×1024 的方形 PNG
2. 替换 `app-icon.png` 或指定路径
3. 运行 `npm run icon` 重新生成
