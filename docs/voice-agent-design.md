# Voice Agent 设计方案（最终版）

> 状态：已确认，准备开发
> 分支：`worktree-voice_agent`

---

## 1. 目标

**按下快捷键 → 说话 → Enter → agent 立即开始干活**。

用户全程不需要切换到终端，不需要点选目标，oc-claw 自动把转写文本送达正在运行的 agent。多 agent 时按"上次用的 / 最近活跃的"自动选；可在设置里固定到某一个或改成每次询问。

## 2. 当前实现现状

| 阶段 | 状态 |
|---|---|
| 全局快捷键 `Ctrl+Shift+V` 注册 | ✅ |
| macOS 原生 STT (`SFSpeechRecognizer`) | ✅ |
| 转写事件流 `voice-status` / `voice-transcript` | ✅ |
| 气泡显示转写 + Esc 取消 | ✅ |
| **Enter 提交真正送达 agent** | ❌ 只有半成品的 OpenClaw 路径，没装 CLI 就静默失败 |

## 3. 体验流（零跳转）

```
[在任何应用里做任何事]
        │
   按 Ctrl+Shift+V（全局快捷键，不抢焦点）
        │
   宠物气泡浮出 "正在听..."
        │
   说话 → 实时流式转写
        │
   停顿 / 再按一次快捷键 / 30s 超时
        │
   ┌──────────────────────────────────────┐
   │ 你好能听到吗                          │  ← 可编辑
   │ ─────────────────                    │
   │ → Claude Code (PawPet)        ⌥ 改   │  ← 自动选好，按住 Option 才显示候选
   └──────────────────────────────────────┘
        │
   按 Enter
        │
   气泡消失
        │
   目标终端：文本被注入 + 自动回车 + agent 开始处理
        │
   用户焦点保持在原处，不被切走
```

## 4. Target 候选源 & 自动识别

### 4.1 自动识别（无需手动配置）

| Agent | 识别机制 |
|---|---|
| **Claude Code** | oc-claw 启动时 `install_claude_hooks()` 写入 `~/.claude/hooks/ooclaw-hook.sh`。CC 启动会话 → hook 通过 Unix socket 通知 → Rust 记录 `ClaudeSession`。 |
| **Codex** | `install_codex_hooks()` → `~/.Codex/hooks/`，同上 |
| **Cursor** | `install_cursor_hooks()` → `~/.cursor/hooks/` + VS Code 扩展（HTTP 端口） |

**前提**：用户至少运行过一次 oc-claw（hook 持久存在，不需要 oc-claw 一直在线）。

### 4.2 需要手动配置

| Agent | 为什么 |
|---|---|
| **OpenClaw** | 远程可扩展平台，需在 settings → `oc_connections` 显式加机器 |

### 4.3 不能识别（本期不支持自动识别）

- opencode / aider / cline / goose / amp / continue.dev 等无 hook 系统的 CLI agent
- 这类 agent 通过**「📋 当前终端」万能通道**支持：用户在该 agent 的终端窗口里触发录音，dispatch 走"注入当前 frontmost 终端"路径

### 4.4 frontmost → target 映射（自动选目标的核心）

```
按下快捷键的瞬间，取 frontmost app + 上下文：

frontmost = Ghostty
  └── 取 active tab 的 cwd
        └── 反查 ClaudeSession (source ∈ {cc, codex}) by cwd
              └── 匹配 → 选它
              └── 不匹配 → 退而求其次：选最近活跃的同源 session

frontmost = iTerm
  └── 同上（iTerm AppleScript 取 active session cwd）

frontmost = Terminal.app
  └── 同上

frontmost = Cursor
  └── 取 active window 的 workspace
        └── 反查 ClaudeSession (source == cursor) by workspace
              └── 同 cwd 匹配规则

frontmost = oc-claw mini 自己
  └── 用 lastUsedTargetId
        └── 没用过 → 选所有 alive session 里 updated_at 最新的

frontmost = 其他（浏览器、IM 等）
  └── 用 lastUsedTargetId
        └── 没用过 → 选所有 alive session 里 updated_at 最新的

无任何 alive session
  └── 显示「未检测到运行中的 agent」+ Enter 走剪贴板
```

## 5. 投递实现（各终端不同手段）

| 宿主 | 焦点抢占 | 实现 |
|---|---|---|
| **iTerm2** (CC/Codex) | ❌ 不需要 | AppleScript：`tell application "iTerm" to tell session id "..." to write text "..."` |
| **Terminal.app** (CC/Codex) | ❌ 不需要 | `do script "..." in window "..."` |
| **Ghostty** (CC/Codex) | ⚠️ 半秒 flicker | 写剪贴板 → `activate Ghostty` → keystroke `cmd+v` → keystroke return → `activate <prev frontmost>` 恢复焦点 |
| **Cursor** | ❌ 不需要 | 扩展 API：`vscode.window.activeTerminal.sendText(message, true)`（true 表示自动回车）。需在 `extensions/cursor/extension.js` 加 `POST /send-terminal-text`。 |
| **OpenClaw** | N/A | `Command::new("openclaw").args(["agent","--message",&msg]).spawn()` |
| **"📋 当前终端"** | 取决于宿主 | 与上面 Ghostty 路径一致（万能 paste），适用 opencode 等无适配 agent |

### 5.1 paste 注入用剪贴板的原因

`osascript keystroke "中文"` 在 macOS 上对非 ASCII 字符不可靠（依赖 keyboard layout，常丢字）。所以中文转写**必须**走"写剪贴板 + ⌘V"。

### 5.2 剪贴板覆盖问题

dispatch 时会临时覆盖用户原剪贴板内容。本期不做恢复（复杂度高），README 注明这一行为。

## 6. Settings 模式（用户可改）

```
Settings → 语音助手
─────────────────────────────────────
🎙️ 投递目标：
    ◉ 自动（最近活跃的 agent）           ← 默认
    ○ 固定到：  [Claude Code (PawPet) ▼]
    ○ 询问每次（编辑气泡里显示候选列表）

🎙️ 当目标失联 / 列表为空：
    ◉ 复制到剪贴板 + Toast 提示            ← 默认
    ○ 不投递，只显示错误
```

### 6.1 "上次用过的"加成

无论哪种模式，记录 `lastVoiceTargetId`：
- 自动模式：候选 `updated_at` 差 < 5 秒时优先 `lastVoiceTargetId`
- 询问模式：默认高亮 `lastVoiceTargetId`

存到 `pet-data.json` 或 `settings.json`。

## 7. 编辑气泡 UI（极简版）

**默认状态（不显示候选列表）**：
```
┌──────────────────────────────────┐
│ 你好能听到吗                      │  ← 可编辑 textarea
│ ────────────                     │
│ → Claude Code (PawPet)    ⌥ 改   │  ← 当前目标小字 + 提示按 Option
└──────────────────────────────────┘
   Enter 发送 · Esc 取消
```

**按住 Option 时（显示候选列表）**：
```
┌──────────────────────────────────┐
│ 你好能听到吗                      │
│ ────────────                     │
│ 选择目标（↑↓ 切换，Enter 确认）   │
│  ● Claude Code  (PawPet)         │
│  ○ Codex        (other-proj)     │
│  ○ Cursor       (workspace)      │
│  ○ 📋 当前终端                   │
└──────────────────────────────────┘
```

释放 Option → 折叠回小字。

## 8. Rust 端 API 设计

### 8.1 `list_voice_targets`

```rust
#[derive(Serialize)]
struct VoiceTarget {
    id: String,             // session_id 或 "openclaw" / "focused_terminal"
    kind: String,           // "cc" | "codex" | "cursor" | "openclaw" | "focused_terminal"
    label: String,          // 展示名，如 "Claude Code (PawPet)"
    cwd: Option<String>,
    updated_at: Option<i64>,
    is_frontmost: bool,     // frontmost match → 自动模式首选
    is_last_used: bool,     // 是否上次用过的
}

#[tauri::command]
async fn list_voice_targets(
    state: tauri::State<'_, ClaudeState>,
) -> Result<Vec<VoiceTarget>, String>
```

排序优先级：`is_frontmost desc > is_last_used desc > updated_at desc`。

### 8.2 `dispatch_voice_message`

```rust
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum VoiceDispatchResult {
    Sent      { target_kind: String, target_label: String },
    Clipboard { reason: String },
    Failed    { reason: String },
}

#[tauri::command]
async fn dispatch_voice_message(
    message: String,
    target_id: Option<String>,
    target_kind: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, ClaudeState>,
) -> Result<VoiceDispatchResult, String>
```

入参约定：
- `target_id = None` ⇒ 走剪贴板（兜底）
- `target_kind == "focused_terminal"` ⇒ 不抢焦点的"当前终端"路径
- 其他 kind ⇒ 按 §5 路由

## 9. 实施步骤（每步可独立验证）

| Step | 内容 | 验收 |
|---|---|---|
| **1** | 更新设计文档（本文） | 用户确认 ✓ |
| **2** | Inventory：读现有 `submitVoicePrompt` / `jump_to_claude_terminal` / `get_active_ghostty_terminal_id`，确定用户用的是 Ghostty 还是 iTerm | 知道走哪条注入路径 |
| **3** | Rust `dispatch_voice_message`（先只剪贴板分支） | Enter 后 `pbpaste` 拿到文本 |
| **4** | Rust `list_voice_targets`（自动选最佳目标） | 控制台打印的目标符合预期 |
| **5** | iTerm AppleScript 注入分支 | iTerm 用户的 CC/Codex 无 flicker 收到文本 |
| **6** | Terminal.app 注入分支 | Terminal.app 用户同上 |
| **7** | Ghostty raise-paste-restore 分支 | Ghostty 用户 flicker 一下能收到 |
| **8** | 前端 `Mini.tsx` UI 改造：转写下显示「→ target」小字 + Option 切换 | 视觉上看到目标，操作流畅 |
| **9** | 前端 toast 反馈（成功 / 剪贴板 / 失败） | 任何路径都有反馈 |
| **10** | Cursor 扩展 `POST /send-terminal-text` | Cursor 用户能收到 |
| **11** | OpenClaw spawn 路径 | 装 openclaw 的用户能用 |
| **12** | Settings UI 三种模式 | 用户可改 |
| **13** | 端到端测试（CC + Codex 现场） | 演示给用户看 |

**本次先做 Step 2-9 + 13**：覆盖到用户当前的 CC/Codex 测试场景。Step 10-12 是后续打磨。

## 10. 边界 & 异常

| 场景 | 行为 |
|---|---|
| 录音中再按一次 Ctrl+Shift+V | 立即停止录音并进入 editing |
| 录音超时 30s | 自动停止 |
| 转写为空 | Enter 置灰 |
| 选中 session 在 editing 中死掉 | dispatch 检测到 → 退回剪贴板 + toast |
| 用户中途关 mini | 视作 Esc 取消 |
| osascript paste 失败（无 Accessibility 权限） | 文本仍进剪贴板 + toast「已复制，⌘V 手动粘贴」 |
| 多个 Ghostty/iTerm 窗口 | 优先 frontmost；若 frontmost 不是该终端，找 cwd 匹配的 tab |

## 11. 风险

1. **Accessibility 权限**：dev 模式继承终端 IDE 权限；打包后 release 必须用户手动授权 oc-claw.app。本次开发先 dev 验，release 前 README 写授权步骤。
2. **iTerm AppleScript 可靠性**：iTerm 必须开启 Python API / AppleScript 集成。如果用户禁用了，回退到 Ghostty 同款的 raise-paste 路径。
3. **session ↔ terminal tab 映射不确定**：CC/Codex 的 hook 不提供 tab 信息。靠 cwd 匹配——多 tab 同 cwd 时取 frontmost / 最近活跃那个。
4. **剪贴板覆盖**：用户在做"复制 → 粘贴"时如果中途触发语音，原剪贴板内容会被覆盖。本期不做恢复。

## 12. 后续优化（不在本期范围）

- 麦克风图标按钮
- 录音音量条 / 波形
- 录音/提交音效
- listen 状态宠物动画
- Cursor 扩展 `terminal.sendText`
- OpenClaw spawn 路径
- Settings UI 三种模式
- 自定义 CLI agent 模板（任意命令）
- 多语言识别
