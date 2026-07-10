# pawbae.ai 官网视觉层 v1 设计

> 适用目录：`apps/website/`。本规格只重做营销官网视觉层，不改变静态导出、候补名单提交逻辑、页面 section 顺序或锚点。

## 0. 用户决策记录（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| 参考体系 | 严格采用 MoneyCoach 的排版尺度、组件语言和深浅条带节奏 |
| Hero 美术 | 方案 A「窗沿上的 Bobo」：自有猫系桌宠趴在真实 Claude Code 终端窗口上 |
| 色彩 | 正文与标题保持 slate/gray；粉彩只用于宠物、状态、胶囊和 visits 天空渐变 |
| 转化目标 | 免费下载优先，主文案统一为 `Download Now For Free` |
| 运行边界 | 无外部字体、CDN、图片外链或新增运行时依赖 |

## 1. 设计方向

页面面向同时使用 Claude Code、Codex 或 Cursor 的开发者。整体感觉应当像一个认真打磨的原生 Mac 产品网站，而不是儿童产品或通用 AI SaaS 模板。

核心记忆点是 Bobo 的轮廓和动作。Bobo 为奶油色猫系桌宠，具有天蓝耳尖、蜜桃色尾环、薰衣草小配件和柔和棕色面部线条。所有插画复用同一头身比例、耳朵轮廓、眼睛和尾巴造型。

设计参数：

- `DESIGN_VARIANCE: 7`：大块留白、少量错位和轻微旋转，但不牺牲阅读顺序。
- `MOTION_INTENSITY: 4`：首屏分层进入、尾巴摆动、按钮和 FAQ 反馈；无滚动劫持或复杂视差。
- `VISUAL_DENSITY: 3`：每个 fold 只讲一个主题，正文最大 65ch。

## 2. 全局视觉系统

- 字体：`-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", "PingFang SC", sans-serif`。终端单独使用系统等宽栈。
- 色彩：Hero `#0a0a08`，深带 `gray-950/900`，正文 `slate-900/600`，品牌蓝 `#0562C0`，主转化黄 `yellow-400`。
- 圆角：按钮 12px，卡片 16px，头像、状态点和公告胶囊为 full radius。
- 卡片：白色 95% 表面、slate 边框和克制阴影；玻璃效果只用于确实叠在彩色或深色背景上的浮层。
- 间距：桌面 section 通常 96-128px 上下留白，移动端 72-88px。内容宽度约 1120-1180px。
- 交互：所有链接、按钮、输入和 summary 有清晰 `:focus-visible`；按钮按下时轻微下沉。

## 3. 页面结构

### 3.1 导航

采用 MoneyCoach 式半透明近白导航，与深色 Hero 构成明确分界。桌面保持单行，移动端隐藏次要锚点，只保留品牌与免费下载安装 CTA。高度不超过 72px。

### 3.2 `download` Hero

左对齐公告胶囊、两行以内 H1、20 词以内说明和双 CTA。主 CTA 使用黄色，副 CTA 使用白色幽灵样式。

下方是一张近全宽产品主视觉：可信的 macOS 终端窗口正在运行 Claude Code，会话包括用户指令、Read/Edit/测试和完成状态。Bobo 趴在窗框上，前爪越过标题栏，尾巴在终端外侧轻摆。浮层只保留一个 working 状态胶囊，不堆叠 toast。

375px 下标题缩至 40px 左右，CTA 纵向排列，终端保持足够宽度并裁去次要右侧内容；宠物缩小并保持完整可见。

### 3.3 工具带

深色 `gray-950` 带只表达兼容性：Claude Code、Codex、Cursor。使用简洁标志胶囊和一句本地 hook 说明，不添加外部 logo 依赖。

### 3.4 `features`

左侧为 kicker、H2、短说明和三条行内加粗功能点。右侧为实际状态舞台，展示 working、waiting、compacting、offline 四种语义状态及 Bobo 的对应动作。状态点颜色分别为薄荷、蜜桃、薰衣草和灰。

### 3.5 `visits`

全页唯一彩色 band，使用天空蓝到薰衣草再到蜜桃的低饱和渐变。右侧上方为好友玻璃列表，头像胶囊和 working/idle/offline 状态点符合产品语义；下方为轻微错位的 App Store 精选式记忆卡，主视觉出现 Bobo 和 Momo 两只统一造型的宠物。

### 3.6 `privacy`

回到深色段落，标题保持 `Local. Private. Yours.`。右侧插画为 Bobo 守在一间简化的本地小屋前，锁形只作为场景道具。三条功能点分别对应 on-device、只共享 mood、访问控制。

### 3.7 `details`

避免四个相同 emoji 卡片。采用 2×2 节奏网格：OBS stream stage 和 pet diary 为较强视觉块，eggs & dex 和 skin workshop 为较轻内容块。图形沿用终端、记忆卡和宠物插画语言，不新增外部资产。

### 3.8 `faq`

浅灰背景，左侧简短标题和 GitHub 引导，右侧保留原生 `details/summary`。summary 点击区至少 48px，高对比焦点环，展开图标在 `+` 与正常连字符形态间切换，不破坏键盘操作。

### 3.9 `waitlist`

深色候补 band。保留 `Waitlist.svelte` 的 env、fetch、409、sending、done 和 error 逻辑，仅重做输入、按钮和反馈样式。输入有显式可访问名称，placeholder 和错误文字达到 AA 对比度。

### 3.10 Footer

白色六列 mega-footer。桌面六列，768px 为三列，375px 为两列。保留现有链接信息架构，用统一 Bobo paw 标志替换占位图形。

## 4. 动效

- Hero 公告、标题、CTA 和主视觉按阅读顺序做一次性 CSS 进入，默认内容始终可见。
- Bobo 尾巴和 working 状态点有低频循环，动画仅作用于 transform/opacity。
- 记忆卡 hover 只做轻微复位和阴影变化，移动端无悬停依赖。
- `prefers-reduced-motion: reduce` 下取消进入、循环和位移动画，仅保留即时状态变化。

## 5. 实现边界

允许修改：

- `apps/website/src/app.css`
- `apps/website/src/routes/+layout.svelte`
- `apps/website/src/routes/+page.svelte`
- `apps/website/src/lib/Waitlist.svelte` 的 markup 文案和样式，不修改提交逻辑
- `apps/website/static/favicon.svg`

禁止修改：

- `apps/website/static/update/latest.json`
- `apps/website/src/routes/+layout.ts`
- adapter-static 配置
- Waitlist 的 env 判定、fetch 目标、请求 headers/body 和状态机语义
- section 顺序及 `download/features/visits/privacy/details/faq/waitlist` id

## 6. 验证

- 在 `apps/website/` 运行 `pnpm install`、`pnpm build`、`pnpm check`。
- 在 375、768、1280、1440 宽度截图，检查每档 `scrollWidth === clientWidth`。
- 与 MoneyCoach 对照导航高度、Hero 字号、CTA 尺寸、section 留白、深浅 band 节奏和 footer 密度。
- 检查所有图片与 SVG 的替代文本策略、焦点可见性、正文 AA 对比度、summary 键盘操作和 reduced-motion。
- 对 `static/update/latest.json` 计算修改前后哈希，必须完全一致。

## 7. 非目标

- 不改产品功能或下载逻辑。
- 不新增页面、路由、国际化系统、分析事件或后端。
- 不引入第三方动画、图标、字体或图片包。
- 不创建 PR；完成后只推送 `design/website-v1`。
