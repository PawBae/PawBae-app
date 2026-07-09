# 每周 Paw 报告分享卡 — 设计

**日期:** 2026-07-08
**背景:** 战略 30 天清单第 5 项（第 3–4 周：周报分享卡，canvas 导出 PNG，en+zh）。公开阶段门槛之一是「每周 3+ 自发晒卡」——这张卡就是被晒的东西。viberank 证明开发者爱晒 agent 账单；差异化在宠物 IP：刷到的人一眼认出「这是 PawBae」，而不是又一张账单截图。

## 用户决策（2026-07-08 敲定）

1. **视觉身份 = 宠物主视觉 + 账单大数字**：宠物立绘 C 位 + hero token 数 + 7 日柱状图 + 连胜徽章。
2. **导出 = 保存 PNG + 复制剪贴板**：预览弹窗两个按钮；复制走新增 `tauri-plugin-clipboard-manager`（微信/小红书直接粘贴）。
3. 入口：Panel 宠物区「📸 周报卡」按钮 → 预览弹窗。

## 卡面（1080×1440 竖版 3:4）

深色底（呼应 app #1a1a20）+ 矢车菊蓝点缀（#6495ED）：

1. 头部：🐾 PawBae 字标 + 周范围（如 7.2–7.8）
2. 宠物立绘：当前形态 idle 首帧，像素风放大（`imageSmoothingEnabled=false`），加载失败回退大 🐾
3. 进化阶段行：stage emoji + 名称（复用 growth.stage.* 文案）
4. **Hero 数字**：本周投喂 tokens（input+output，不计 cache，与喂养循环口径一致），en 用 K/M/B、zh 用 万/亿
5. 7 日迷你柱状图（零数据周显示底座条，不空白）
6. 数据行：🤖 N 次任务 · 💬 N 消息；连胜行：🔥 N 天连胜 + 🛡️×n（连胜 0 则整行省略）
7. 底部：「和 Yoonie 相伴第 N 天」+ pawbae.ai 水印

零数据周照常出卡（宠物、连胜、相伴天数还在）——刚装的用户也能晒。宠物名 v1 用 pet displayName（起名功能落地后自动替换）。

## 架构（三层）

1. **纯装配器 `utils/weekly-report.ts`**：`assembleWeeklyReport({statsList, recentAwards, petData 切片, stageIndex, petName, lang, now})` → 扁平 `WeeklyReport`。
   - 数据源：`get_claude_stats` 已返回每源 **14 天逐日** `daily_stats`（**线格式 snake_case**：`input_tokens` 等，无 serde rename——照实补 TS 类型）；三源按日期字符串对齐求和，取末 7 天；某源 fetch 失败（null）当零处理。
   - 任务数：ledger `recent` 里 7 天内的 `agent_stop` 条目数；`recent` 上限 100 条，打满时卡上显示 `100+`（`tasksCapped` 标志），不谎报。
   - 数字格式化 + 周范围文案都在这层。**全部单测**。
2. **哑渲染器 `utils/share-card.ts`**：`renderShareCard(canvas, report, labels, sprite?)` —— 只画不算；文案由调用方从 i18n 取好传入；sprite 参数 `{image, sx, sy, sw, sh} | null`。
3. **`ShareCardModal.svelte`**：打开时并行拉三源 stats → 装配 → 渲染到预览 canvas。按钮：保存（`@tauri-apps/plugin-dialog` save 对话框 → canvas.toBlob → base64 → Rust `save_png_file` 写盘）、复制（`Image.fromBytes(png)` → `writeImage`，tauri 已开 `image-png` feature）、关闭。

## Rust / 权限增量

- Cargo：`tauri-plugin-clipboard-manager = "2"`；lib.rs 注册。
- 新命令 `save_png_file(path, base64)`（commands/misc.rs）。
- capabilities：`dialog:default`、`clipboard-manager:allow-write-image`。
- npm：`@tauri-apps/plugin-dialog`、`@tauri-apps/plugin-clipboard-manager`。

## 遥测（进事件字典）

`share_card_export {method: save|copy}` —— 直接量化「每周 3+ 自发晒卡」门槛。打开预览不上报，只报导出动作。

## 非目标（v1）

- 不做卡面主题/配色切换、不做历史周回看、不做自动周日弹窗提醒（留存数据出来再议）。
- 不做直接分享到平台的 SDK 集成——保存/粘贴已覆盖国内外主流路径。
- 全勤天数本周统计不上卡（board 历史不持久化，只有当日）——加历史属另一个功能。
