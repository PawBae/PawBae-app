<p align="center">
  <img src="src-tauri/icons/icon.png" width="80" />
</p>
<h1 align="center">PawBae</h1>
<p align="center">
  <a href="./README.md">English</a> | <b>中文</b>
</p>
<p align="center">
  AI 驱动的桌面宠物，实时监控你的编程 Agent。
</p>

## 功能

- **桌面宠物** — 精灵帧动画的桌面伙伴
- **Agent 监控** — 实时响应 Claude Code、Codex、Cursor 的活动状态（工作中、空闲、等待审批）
- **消息气泡** — 显示 Agent 状态通知和 AI 对话回复
- **可交互** — 点击、拖拽，与宠物互动
- **宠物养成** — 饥饿度、好感度、金币、喂食、番茄钟

## 技术栈

- **前端**：Svelte 5 + TypeScript + Vite
- **桌面框架**：Tauri 2（Rust）
- **样式**：Tailwind CSS

## 环境要求

- macOS 或 Windows
- 安装 [Claude Code](https://docs.anthropic.com/en/docs/claude-code)、[Codex](https://github.com/openai/codex) 和/或 [Cursor](https://www.cursor.com)（用于 Agent 监控）

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 构建

```bash
pnpm tauri build
```

## 许可证

MIT
