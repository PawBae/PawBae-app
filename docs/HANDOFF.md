# PawBae 交接文档

**快照日期:2026-07-13(由主会话按 v1 发布计划 §0 的"里程碑对账"职责刷新;上一版 2026-07-08 已过时)**

给接手/协作的人:本文档是项目状态的入口快照。配合阅读:`CLAUDE.md`(工程约定与踩坑)、`docs/strategy/2026-07-07-startup-strategy.md`(创业战略)、`docs/strategy/2026-07-09-v1-release-plan.md`(v1 十周发布计划,当前执行基准)、`docs/team/line-{a,b,c}-*.md`(三条线分工;注意其勾选框普遍落后于实际,以 merged PR 为准)。

## 项目一句话

PawBae 把 AI agent 包装成桌面宠物("你的 AI agent 的脸"),让 agent 对普通人变得亲民;当前用户是 coding agent 开发者,v1 的发布底线是 **P4-B 桌面社交串门**(好友互访 + 共同记忆)。

## 当前位置:v1 十周计划的 W10 / M6 门槛

十周计划(2026-07-09 定稿)的 M1–M5 已全部落地(实际只用了约 4 天,PR #54–#75):

| 里程碑 | 状态 | 证据 |
|--------|------|------|
| M1 地基(monorepo/云 schema/更新器验签/崩溃上报) | ✅ | PR #55/#58/#59/#60,supabase/migrations |
| M2 桌面登录 + 官网 | ✅ | PR #61(GitHub OAuth 桌面流),apps/website + join_waitlist RPC |
| M3 好友关系 e2e | ✅ | PR #64–#66(投影管道、SupabasePlatformClient) |
| M4 串门 aha(真实栈) | ✅ | PR #67/#68(真实 PlatformClient 换线) |
| M5 共同记忆闭环 | ✅ | PR #71/#72(SV §12 异常矩阵 + 记忆客户端管道) |
| **M6 封闭内测发布** | ⏳ **当前门槛** | 见下 |

## M6 发布门槛对账(2026-07-13)

工程验收(发布计划 §7):

- ✅ 测试基线:vitest **407/407**(要求 360+)、svelte-check 0 错误、biome/clippy/rustfmt 经 CI 全绿(最近 push #75 成功)
- ✅ RLS 完整矩阵(硬门槛):`supabase/tests/005_security_matrix_test.sql` + 000–004 阶梯 + api/realtime e2e 脚本
- ✅ SV §12 测试地图:PR #71 对真实栈执行断网/重启/双端竞态矩阵
- ✅ 发布链就绪:macOS Developer ID 签名 + 公证(PR #69/#70)、minisign 验签(#59/#62)、版本单源 bump 脚本 + latest.json 自动生成(#73)、生产部署管道 cloud-deploy.yml + runbook(#74);全部 9 个 release secrets 已配置(Apple 全套 + MINISIGN + APTABASE,2026-07-11)
- ⏳ **签名安装包出厂:未发生**——GitHub Releases 只有 6 月的两个 v0.2.0/v0.1.0 草稿,`tauri.conf.json` 版本仍是 0.2.0。发布链从建成后没有真正跑过一次正式版本
- ⏳ **最终验收(人)**:两位真实用户从互加好友到完成串门、全程无开发工具(SV §12.4)——依赖创始人物色两对好友(发布计划 §9.4)

产品门槛(SV §9 五步漏斗:接受 ≥40% / 首访 ≥35% / 完成 ≥70% / 复访 ≥25% / 举报 <1%)在内测开始后用 A 线的 funnel SQL 视图核对,不在发布前。

## 接下来(按序)

**AI/工程可做:**
1. 按 `docs/RELEASING.md` runbook 出 **v0.3.0**:跑版本 bump 脚本 → 发布 PR → tag → release.yml 产出签名安装包草稿 + latest.json(发布 draft 是安全操作,公开发布由创始人点)
2. 发布前冻结窗口只收修复(W10 规则);三条线文档里的陈旧勾选框建议各 owner 对照 merged PR 清一遍
3. 内测开始后:盯 funnel 视图 + 崩溃上报,凑首个"修情感质量"迭代(SV §9:只完成一次不复访 → 修共同记忆质量,不加奖励)

**只有创始人能做(发布计划 §9 余项):**
1. 物色两对真实好友对测(M6 最终验收的硬前置)
2. 公开发布 v0.3.0 release(点 publish)+ 确认 pawbae.ai DNS→Vercel 切换状态(§9.2,更新器 URL 依赖它)
3. (可选)Azure Trusted Signing 账号(Windows 签名;首波可带说明先行)

## 三条线状态速览

- **A 云端**:schema/RLS/RPC/限速/记忆结算/funnel 视图已 merge;生产库部署管道(#74)刚修复"生产库为空"的发布阻断
- **B 连接**:账号/心跳/投影/真实 PlatformClient/发布链全 merge;PR #75 已做 B 线工程交接
- **C 舞台**:官网 + waitlist(走 join_waitlist RPC,防探测设计)、四宠 onboarding、社交 Home、记忆卡已 merge

## 工程速览(monorepo)

- 布局:`apps/desktop`(Tauri 桌面)、`apps/website`(SvelteKit 官网)、`packages/shared`(契约)、`supabase/`(迁移+测试);根 `package.json` 脚本转发(`pnpm test` → apps/desktop)
- 常用:`pnpm install`、`pnpm test:ci`、`pnpm check`、`pnpm tauri dev`;云端测试见 `supabase/tests/`
- 约定:**永不直推 main**(有仓库规则强制),功能分支 + `gh pr create`;游戏逻辑纯 TS + TDD(`rewards.ts`/`token-feed.ts`/`diary.ts` 等为范例);热点文件所有权与跨线规则见发布计划 §4
- 踩坑史:`CLAUDE.md`(hook/PID/轮询/Windows);另注意 repo 根部的未跟踪 `src-tauri/` 目录是 monorepo 迁移残留,勿误提交

## 历史存档

2026-07-07 首版交接(战略落库 + token 喂养循环)与 2026-07-08 更新(进食渲染修复、遥测/审批单/任务板/周报卡/命名)的内容已全部合入 main 并被上表覆盖;设计文档存于 `docs/superpowers/specs/`。
