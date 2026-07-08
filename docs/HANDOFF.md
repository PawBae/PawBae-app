# PawBae 交接文档(2026-07-07)

给接手开发的人:这份文档是当前项目状态的完整快照——愿景、战略、进行中的工作、验证状态和下一步。配合 `CLAUDE.md`(工程约定与踩坑记录)、`ROADMAP.md`(产品路线)和 `docs/strategy/2026-07-07-startup-strategy.md`(创业战略)一起读。

## 项目是什么

PawBae 是 Yining 的创业项目:AI agent 发展很快但只有少数人在用,大部分人不习惯或不知道从哪开始——agent 不够"亲民"。PawBae 把 agent 包装成桌面宠物,让人爱上使用 agent。

- **当前形态**:Svelte 5 + Tauri 2 桌面宠物(纯 SPA,非 SvelteKit),监控 Claude Code / Codex / Cursor 并实时反应(工作/等待/完成状态、完成音效、语音互动、音乐感知),有完整养成系统(饥饿/好感/金币/进化/成就)。macOS 为主,Windows 接近同等。
- **当前用户**:跑 coding agent 的开发者。
- **战略方向**:开发者是分发渠道不是终点;宠物最终要成为普通人使用 agent 的**界面**。定位一句话:"你的 AI agent 的脸",不是 agent 监控器(监控已被免费克隆商品化)。

## 战略核心(详见 strategy 文档)

- **核心循环「照护倒置」**:宠物的成长绑定真实完成的 agent 工作(不是聊天量)。任务完成 → 宠物开饭/带回纪念品 → 进化 → 晒卡传播。
- **三条设计红线**:永不惩罚、永不打扰、远离恋爱/NSFW。
- **商业模式**:核心永久免费 → Steam 化妆品($3.99–7.99)→ Pro $8/月(BYOK,绝不打包 token)→ 远期消费订阅 + B2B。
- **最优先的非功能工作**:代码签名公证、匿名 opt-in 遥测、Steam 页面。没有这三样,增长和数据都无从谈起。
- **六个已知盲点**(strategy 文档 §9)——接手后建议先做盲点 1(15 人假门测试)和盲点 4(遥测)。

## 进行中的工作:Token 喂养循环(feat/token-feeding-loop 分支)

路线图阶段一的第一个功能,**代码已完成,PR 待合并**。

- **功能**:agent 真正完成任务时,按该来源自上次进餐以来的 token 增量(input+output,不计 cache)给宠物开饭——零食 ≥2k / 正餐 ≥60k / 大餐 ≥300k,恢复对应饥饿度并播放进食动画。不足零食的碎屑累积。免费(不动金币账本,金币仍由既有 agent_stop 路径发)。token 消耗**永不**扣饥饿度。
- **实现**:纯 reducer `src/lib/utils/token-feed.ts`(基线水位 + delta 结算,启动时经 `get_claude_stats` 预热,内存态)+ `pet.svelte.ts` 在 `claude-task-complete` 事件上结算。设计文档:`docs/superpowers/specs/2026-07-07-token-feeding-loop-design.md`。
- **测试**:vitest 194/194 全绿(含 12 个 reducer 测试 + 3 个 store 胶水测试),svelte-check 0 错误,biome 干净(仓库既有 1 个无关警告)。零 Rust 改动。

### 验证状态(诚实版)

- ✅ 单元/类型/lint 全绿。
- ✅ 实跑链路验证:dev app 运行中,通过 `/tmp/ooclaw-claude.sock` 注入合成 hook 事件序列(UserPromptSubmit → Stop),Rust 日志确认事件按预期流转(processing → stopped,触发 claude-task-complete)。
- ⚠️ **进食动画的视觉确认未完成**:宠物窗口被全屏 Space 遮挡时 webview 渲染节流,窗口截图是冻结帧,无法确认 eat 精灵帧。接手后建议:在可见桌面上跑 `pnpm tauri dev`,完成一个真实 CC 任务(或按下面的方法注入合成事件),肉眼确认宠物进食 3 秒。
- 合成事件注入方法(app 运行时):
  ```bash
  printf '%s' '{"sessionId":"test-1","cwd":"/tmp","event":"UserPromptSubmit","claudeStatus":"running","interactive":true,"pid":1}' | nc -U /tmp/ooclaw-claude.sock
  sleep 1
  printf '%s' '{"sessionId":"test-1","cwd":"/tmp","event":"Stop","claudeStatus":"waiting_for_input","interactive":true,"pid":1}' | nc -U /tmp/ooclaw-claude.sock
  ```
  注意:要看到"大餐",两次注入之间该来源需有 ≥2k 的真实 token 增量(baseline 在 app 启动时预热);连续注入第二次只会是碎屑,不会重复进食(这是防刷设计,符合预期)。

## 工程速览(给新人)

- 事件流:hook 脚本(app 启动时自动重装)→ Unix socket → Rust `event_process.rs`(过滤子代理/ESC/压缩)→ Tauri 事件 → Svelte stores。
- 游戏逻辑刻意保持纯 TS + 单测(`rewards.ts`、`evolution.ts`、`token-feed.ts` 等),与 I/O 分离——新功能请沿用这个模式(TDD)。
- 工作流:**永不直推 main**(本次交接文档经 owner 明确授权例外),功能走分支 + `gh pr create`。
- 常用命令:`pnpm install`、`pnpm vitest run`、`pnpm check`、`pnpm lint`、`pnpm tauri dev`。
- 踩坑记录都在 `CLAUDE.md`(hook/PID/轮询/Windows 平台等,是血泪史,改相关代码前必读)。

## 建议的下一步(按优先级)

1. 合并 token 喂养循环 PR(先补上面那步视觉确认);
2. 遥测 + 签名(战略 30 天清单第 1–2 项);
3. 叼来审批单 [S](终端聚焦代码已有,`pet.svelte.ts` 已处理 waiting 事件,纯增量);
4. 每日任务板 + 宽容连胜 [S](成就系统已有,照抄模式);
5. 周报分享卡 [S](metrics 数据已解析,canvas 导出 PNG);
6. 给角色起名 + 写 lore(一个下午,护城河第一块砖)。
