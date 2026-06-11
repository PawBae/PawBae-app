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
- **可交互** — 点击、拖拽、抛掷，与宠物互动
- **宠物养成** — 饥饿度、好感度、金币、喂食、番茄钟
- **自动更新** — 检测新版本并原地升级

## 安装

从 [Releases](https://github.com/PawBae/PawBae-app/releases/latest) 下载最新版本：

| 平台 | 文件 |
| --- | --- |
| macOS（Apple Silicon） | `PawBae_x.y.z_aarch64.dmg` |
| macOS（Intel） | `PawBae_x.y.z_x64.dmg` |
| Windows 10/11 | `PawBae_x.y.z_x64-setup.exe` |

当前版本**未做代码签名**，首次启动时系统会有警告：

- **macOS**：如果提示"PawBae 已损坏/无法打开"，在 **系统设置 → 隐私与安全性 → 仍要打开** 中放行，或清除隔离属性：

  ```bash
  xattr -cr /Applications/PawBae.app
  ```

- **Windows**：在 SmartScreen 弹窗中选择 **更多信息 → 仍要运行**。

## 快速上手

1. 启动 PawBae，选择模式——**编程模式**（Agent 监控）或**宠物模式**（纯养成）。之后可随时在设置中切换。
2. 要监控 Agent，在 **设置** 中打开你使用的集成（Claude Code / Codex / Cursor）。打开开关即自动安装对应工具的 Hook。
3. 在 Agent 的终端里发送一条消息——宠物就会开始响应它的状态。

### 集成会安装什么

Hook 是一些小脚本，通过**本地 socket** 把 Agent 的生命周期事件转发给 PawBae——宠物因此知道 Agent 正在思考、干活还是等待审批。

| 集成 | 写入的文件 |
| --- | --- |
| Claude Code | `~/.claude/hooks/ooclaw-hook.sh`（Windows 为 `.ps1`），注册到 `~/.claude/settings.json` |
| Codex | `~/.Codex/hooks/ooclaw-codex-hook.sh`，注册到 `~/.Codex/hooks.json`（macOS） |
| Cursor | `~/.cursor/hooks/` 脚本，注册到 `~/.cursor/hooks.json`，另含 `pawbae.terminal-focus` 扩展（位于 `~/.cursor/extensions/`） |

Hook 脚本在每次应用启动时重新生成；PawBae 未运行时它们会立即退出，不影响 Agent。

macOS 上可选的**输入感应**功能（宠物对你的打字/点击做出反应）需要**辅助功能**权限，可在 **设置 → 隐私** 中关闭。

## 隐私

所有数据都留在你的电脑上：

- Agent 事件走本地 socket，会话从本地文件读取，不会上传到任何地方。
- 输入感应（macOS，可选）只统计按键/点击的*次数*——不记录按了哪个键、输入了什么内容、点了哪里。
- PawBae 仅有的网络请求是更新检查（`pawbae.ai`）和你确认后的更新下载。

## 卸载

1. 删除应用（`/Applications/PawBae.app`，Windows 在系统设置中卸载）。
2. 如需彻底清理，删除上表中的 Hook 文件及应用数据目录
   （macOS：`~/Library/Application Support/ai.pawbae.app`，Windows：`%APPDATA%\ai.pawbae.app`）。

## 技术栈

- **前端**：Svelte 5 + TypeScript + Vite
- **桌面框架**：Tauri 2（Rust）

## 开发

```bash
pnpm install
pnpm tauri dev
```

本地构建用 `pnpm tauri build`。发布流程与版本规范见 [docs/RELEASING.md](./docs/RELEASING.md)。

## 许可证

MIT
