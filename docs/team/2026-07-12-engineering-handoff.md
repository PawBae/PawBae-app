# 工程交接 · B 线 + 跨线支援（2026-07-12）

写给接班的工程 agent：你接手的是 Claude 代管的 B 连接线（原 @miaomeng1）+ 零散跨线支援。
本文档自足——你没有前任的会话记忆，**先读完这一份，再按 §7 的地图按需读其它文档**。

## 0. 工作方式与红线（每一条都有事故背书，别测试它们）

**Git/PR 流程**

- **绝不直推 main**。所有变更走分支 → `gh pr create`（PR 标题/正文中文，正文以
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)` 结尾）。
- **Yining 亲手合并 PR**，你不合并。CI 全绿 + Codex 意见处理完后报告「整装待合」。
- commit message 以 `Co-Authored-By: <你的署名>` 结尾。
- **禁止 `git commit --amend` + force push**（权限被拒）。要重触发 CI 用空提交。
- 禁止裸 `git stash` / `stash pop`——要用 `git stash push -u -m "<tag>"`，按 SHA apply。
- 禁止 `git add -A`——永远显式列出文件。
- **cwd 会漂移**（本仓库吃过两次亏，一次差点污染用户主工作区）：每个 git/构建命令都
  显式 `cd` 绝对路径或用 `git -C`。用户的主检出（仓库根）**永远停在 main**；开发在
  worktree（前任用 `.claude/worktrees/pr-a-monorepo`，你可以建自己的）。
- HTTPS push 偶发 http2 sideband 断连：重试加 `-c http.version=HTTP/1.1 -c http.postBuffer=157286400`。

**Codex bot 评审**

- 每个 PR 会收到 chatgpt-codex-connector 的行间评论。逐条技术性核实（不盲从），修复后回复，
  回复以 `_🤖 Addressed by [Claude Code](https://claude.com/claude-code)_` 结尾，然后关线程：

  ```bash
  gh api repos/PawBae/PawBae-app/pulls/<PR>/comments/<COMMENT_ID>/replies -f body="..."
  # 找 thread id 并 resolve：
  gh api graphql -f query='query { repository(owner: "PawBae", name: "PawBae-app") { pullRequest(number: <PR>) { reviewThreads(first: 20) { nodes { id isResolved comments(first: 1) { nodes { databaseId } } } } } } }'
  gh api graphql -f query='mutation { resolveReviewThread(input: { threadId: "<THREAD_ID>" }) { thread { isResolved } } }'
  ```

**Shell 纪律**

- **门禁/验证命令绝不 pipe 给 `| tail`/`| head`**——会吞退出码和错误（曾让 lint 违规漏进 CI）。
  裸跑，或输出进文件再看。
- `</dev/urandom | head` 在 `set -o pipefail` 下 SIGPIPE(141) 静默死——随机串用
  `node -e 'crypto.randomUUID()'`。

**凭据边界**

- minisign 私钥只在 GitHub secret `MINISIGN_SECRET_KEY`；Apple 六件套、`SUPABASE_DB_URL`
  只在 GitHub secrets。**绝不落盘/进聊天/进日志**。publishable key 公开设计（RLS 是边界）。
- 凡是「输入密码/证书/付款/建账号」的动作，Yining 亲手做，你给命令和走查。
- 对其它仓库开 PR 需要 Yining 点名那个仓库。

**验证命令**（与 ci.yml 一致；在 `apps/desktop` 或仓库根跑）

```bash
pnpm test          # vitest（当前 541 pass + 15 skip）
pnpm check         # svelte-check（0 errors 基线）
pnpm lint          # biome（apps/desktop/src）
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
pnpm bump --check  # 四处版本号一致性
```

## 1. 项目一分钟

PawBae：coding agent（Claude Code/Codex/Cursor）→ 桌面宠物，正在平台化。
v1 = 平台账号 + 好友 + 异步串门 + 共同记忆，三人十周到封闭内测（时钟 2026-07-09 启动，
W10 = M6 发布）。**P4-B 串门是不可妥协的发布底线**；P4-C 共同记忆可滑出（数据面已做完）。

三线：A 云端 = zhihao（`supabase/` + `packages/shared` 契约 + `cloud.yml`）；
B 连接 = 你（`apps/desktop` 账号/数据管道 + `src-tauri` + 发布链 workflows）；
C 舞台 = Yining（官网、串门渲染、记忆卡视觉）。领地边界见三份 line 文档（§7）。

## 2. 已交付账本（B 线全清单，全部已合并）

| PR | 内容 |
| --- | --- |
| #56 | W1 monorepo 改造（apps/desktop + workspace + CI 路径） |
| #57/#58 | 串门客户端逻辑层平移 + 官网脚手架集成 |
| #59/#60/#62 | W2 更新器 minisign 验签（P0）+ 崩溃上报 + 真公钥（keyid 8ea0b677f40f6a5f） |
| #61 | W3 GitHub OAuth 桌面流（PKCE + loopback 127.0.0.1:53682）+ AccountSection + opt-in 开关组 |
| #63 | W4 connector 三重门 + 心跳 60s + 事件上传 |
| #64 | W5 投影发布管道（去重/3s 间隔/租约守门） |
| #65 | Codex 四宠 onboarding + SocialHome 移植 |
| #66 | W5-6 SupabasePlatformClient 真实实现（19 单测） |
| #67 | W6 真栈联调套件（7 步链路 + `scripts/w6-supabase-integration.sh`） |
| #68 | W7 DI 换线 + Home 灌真实社交数据 |
| #69/#70 | W7-8 Apple 签名+公证接线 + artifacts 修复（端到端验收过：公证 Accepted） |
| #71 | W9 SV §12 恢复矩阵（7 场景 + `scripts/w9-recovery-matrix.sh`） |
| #72 | W9 共同记忆客户端管道（契约扩面 + 终局结算钩子 + 相册真数据） |
| #73 | 版本 bump 脚本（`pnpm bump`）+ tag 版本门禁 + CI 自动生成 latest.json |
| #74 | 生产部署管道 cloud-deploy.yml + runbook（代 A 线；本文写作时待合并） |

**发布链现状**：`pnpm bump X.Y.Z` → PR → 打 tag → 四道门禁（版本一致/minisign/Apple
六件套/公证）→ draft release 自带三平台签名包 + 现成 latest.json → 人工 publish +
部署清单到 pawbae.ai。全链路 2026-07-11/12 实测过。

## 3. 发布差距快照（2026-07-12 实测，含证据）

1. **生产 Supabase 是空库**（最大阻断，A 线域）：publishable key 有效（auth 健康 200），
   但 `join_waitlist` 对外 404——仓库全部迁移从未上云。修复路径 = #74 的 cloud-deploy
   管道，见 §4 第一项。
2. **pawbae.ai 官网是空壳**（C 线域）：首页 504 字节，无候补名单/无下载。材料齐：
   `apps/website` 脚手架 + 候补名单接线（走 `join_waitlist` RPC）在仓库，Codex 视觉稿在
   `design/website-v1` 分支——差合并部署 + Vercel cutover（更新清单托管随之切）。
3. **双人真机实测从未做过**（M2/M5 人证）：两台机器两个 GitHub 账号跑
   登录→兑邀请码→加好友→串门→召回→相册出记忆。`recall→recalled` ~75s 内收敛同时证明
   生产 pg_cron 活着。Yining 本机 `pnpm tauri dev` 登录冒烟也一直没做。
4. **线上 latest.json 旧账**：0.2.0、缺 macos-x64 槽、无签名、`ui.petdex→codexpet.xyz`
   残留。App 源码已确认不消费 petdex，CI 新清单天然不含——**发布即自动解决，无需动作**。
5. 可滑出/非阻断：记忆卡详情视图（C 线；数据管道已通、`@pawbae/shared` 双语文案表就绪，
   Home 侧 `openMemory` 目前只记漏斗+收起事件卡）；Windows v1 不签名（已决议）；
   manbo 皮肤音效授权待确权；A 线「RLS 完整矩阵/限速全覆盖」勾选状态需 zhihao 确认。

## 4. 接手即做的任务队列（按优先级）

1. **执行首次生产部署**（等两个前提：Yining 合并 #74 + 设好 `SUPABASE_DB_URL` secret）：
   ```bash
   gh workflow run "cloud deploy" --repo PawBae/PawBae-app -f action=dry-run
   # 看 run 输出的迁移清单（应为 5 条 20260710*）无异常后：
   gh workflow run "cloud deploy" --repo PawBae/PawBae-app -f action=deploy -f seed_invites=10
   # 盯完四重验收（迁移对齐/17 函数/pg_cron=2/REST 冒烟），
   # 从 run 页面下载 invite-codes artifact 交给 Yining（3 天过期）
   ```
   全部细节在 [docs/DEPLOYING-CLOUD.md](../DEPLOYING-CLOUD.md)。
2. **支援双人真机实测**：部署完成后组织 Yining + zhihao 跑 §3.3 的旅程；出问题按
   `scripts/w9-recovery-matrix.sh` 的场景定位（客户端收敛问题大概率在轮询/广播两腿）。
3. **官网上线支援**（C 线主导，你搭手）：把 `design/website-v1` rebase 到 main、
   接 `apps/website` 部署位、候补名单打真实生产 RPC、Vercel cutover 时**先迁
   `/update/latest.json` 再切 DNS**（更新器硬编码该 URL，顺序反了老用户 404）。
4. **发布日执行**：§2 末尾的发布链流程照跑；CHANGELOG 记得写。
5. （C 线授权后）记忆卡详情视图：用 `memoryCardCopy()`（`social-home.ts`）的
   title+body 渲染，别再造文案源。

## 5. 高频技术地雷（按域分组，全部踩过或实证过）

**本地 supabase 栈**（联调必备）

- CLI 走 `pnpm dlx supabase`（未全局装）；Docker Desktop 先启动；`supabase start`
  自动应用迁移；API 54321 / DB 54322；db 容器名从 `supabase/config.toml` 的
  `project_id` 推导（`supabase_db_pawbae-line-a`），别全局抓第一个匹配。
- 两份联调套件：`bash scripts/w6-supabase-integration.sh`（8 步全链路）、
  `bash scripts/w9-recovery-matrix.sh`（7 场景恢复矩阵）。共用底座在
  `apps/desktop/src/lib/platform/integration-harness.ts`。
- **时间快进机制**：只拨 visits 时钟列 + 手动 `SELECT private.maintain_visits()`（与生产
  cron 同一函数）。时钟列必须**成对整体平移**（租约恰 30min、请求窗恰 24h 的跨列不变量，
  拨单列会让 maintain 在触发器里炸掉且一行坏数据拖垮整个 cron）。
- A 线的 api-e2e 与 realtime-e2e **不能并行跑**（竞态用例互相干扰）。

**客户端架构不变量**

- 轮询祖训：busy lock 防重入；失败静默等下一 tick；**绝不 stale-discard**。
- Broadcast 不回放：测试里发布前必须 `untilJoined()` 等频道 join；生产靠轮询兜底。
- 幂等键随「用户意图」，重试复用；服务端结算按 visit_id 唯一。
- 契约单一来源：数据类型从 `@pawbae/shared` re-export（`platform/types.ts`）；
  PlatformClient 接口归 B，变更需 C review。
- 隐私红线：上传只走 shared 字典构造器；投影 5 枚举；记忆只有模板键+安全参数，
  **不存在自由文本上云的代码路径**——review 时死守。

**发布链**

- tauri-action 输入名会漂移（老输入被静默忽略只报 warning）——升级后核对。
- Apple 公证首次提交深扫约 4.5h（一次性），之后 ~25s/次；别在首次时误判卡死。
- `APPLE_CERTIFICATE` 用 `base64 -i x.p12 | gh secret set ...` 直管（剪贴板会污染）。
- 本机验证签名缺 Developer ID G2 中间证书会「0 valid identities」：
  `curl -O https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer && open` 装入。
- upload-artifact 混合相对/绝对路径会推错公共根——按平台拆步。

**Windows 专项**：见 [CLAUDE.md](../../CLAUDE.md)「Windows Platform」段（DPI/路径/hooks/
音频/构建锁文件），全部来自前代项目实账。

## 6. 与 Yining 的协作模式

- 他的消息极短（「XX合并了 你继续」= 按看板推进下一项，不用等确认）；提问才是提问。
- 报告风格：先结论后细节，中文，PR 链接可点；「待你做的」单列。
- 他亲手做的事：合并 PR、一切凭据操作、产品拍板（拍板项要单独列出等他一句话）。
- 进度记忆：前任维护在 Claude 侧 memory（你没有）——**本文档就是快照**；接手后你自己
  建你的进度记录方式，并在大节点更新本文档或写新交接。

## 7. 文档地图

| 文档 | 什么时候读 |
| --- | --- |
| [line-b-connector.md](line-b-connector.md) | 你的领地/逐周看板（已按事实全勾，唯 Windows 签名开放） |
| [line-a-cloud.md](line-a-cloud.md) / [line-c-stage.md](line-c-stage.md) | 跨线协作前看边界 |
| [v1 发布计划](../strategy/2026-07-09-v1-release-plan.md) | 总纲/里程碑/滑动规则 |
| [SV spec](../superpowers/specs/2026-07-09-social-visiting-design.md) | 串门/记忆的产品真理（§12 是验收线） |
| [RELEASING.md](../RELEASING.md) | 出版本操作手册（bump→tag→publish→清单） |
| [DEPLOYING-CLOUD.md](../DEPLOYING-CLOUD.md) | 生产云端部署/邀请码播种 |
| [CLAUDE.md](../../CLAUDE.md) | 仓库级规范 + 前代项目地雷 |
