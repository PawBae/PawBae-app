# 匿名 opt-in 遥测（Aptabase）— 设计

**日期:** 2026-07-08
**背景:** 战略 30 天清单第 2 项 —— 没有安装数 / D1/D7/D30 / 日均 agent 会话数据，所有阶段门槛都是盲猜。创始人已选定 Aptabase（开源、隐私优先、Tauri 官方插件、可自托管）。

## 原则（不可协商）

1. **Opt-in，默认关。** 战略文档措辞是"匿名 opt-in 遥测"。任何事件在用户明确打开开关前一个都不发。
2. **匿名。** 事件属性只允许枚举值和数字。永不发送：session id、cwd、prompt 内容、窗口标题、终端内容、昵称、任何路径。
3. **插件零自动采集。** tauri-plugin-aptabase 不自动上报任何事件（README 明确），所以 opt-in 门控放在前端唯一的 `track()` 出口即可覆盖全部。

## 架构

```
事件源(前端) → utils/telemetry.ts track() ——门控 settingsStore.telemetryEnabled
                                  │ 通过
                                  ▼
                    @aptabase/tauri trackEvent() → Rust 插件(批量队列) → Aptabase
```

- **密钥编译期注入**：`option_env!("APTABASE_APP_KEY")`。无密钥的本地 dev 构建**不注册插件**，前端 `track()` 静默吞掉 missing-plugin 错误 —— 开发环境天然零上报。发布构建由 CI secret `APTABASE_APP_KEY` 提供（release.yml 已加 env 透传）。
- **全部 track 调用都在前端**，因为 opt-in 开关存在 settings.json、由前端 settings store 拥有。Rust 侧只负责插件注册。

## 事件字典（v1 —— 刻意最小）

| 事件 | 属性 | 回答什么问题 |
|---|---|---|
| `app_started` | `mode`: coding/pet | 安装数、DAU、D1/D7/D30 留存（Aptabase 按会话自动算） |
| `agent_task_complete` | `source`: cc/codex/cursor | 日均 agent 会话/任务量 —— 核心循环是否被真实使用 |
| `meal_fed` | `tier`: snack/meal/feast | token 喂养循环参与度 |
| `approval_response` | `awarded`: 0/2 | 叼来审批单是否改变了响应行为 |

加新事件 = 在此表加一行 + 一处 `track()` 调用。不进此表的事件不许发。

## UI

- **设置 → 隐私**：「匿名使用统计」开关（默认关），文案说明只发匿名计数、可随时关闭。
- **Onboarding**：模式选择卡片下方一个**默认不勾选**的复选框（真 opt-in，不做预勾选的假 opt-in）。新用户是安装数的主要来源，这里是唯一一次自然的询问机会。

## 非目标（v1）

- 不做错误/panic 上报（Aptabase 支持但另议——错误栈可能含路径）。
- 不做 Rust 侧事件、不做 `app_exited`（长驻应用退出即卸载信号弱，且退出 flush 有挂起风险）。
- 不做自托管配置面板 —— 密钥换成 `A-SH-*` 即自托管，无代码改动。
