# 皮肤工坊 UGC v1（本地工坊 MVP）设计

> 阶段二头号项目。战略依据：VPet 靠 745+ 模组换来 5 万条 98% 好评，Desktop Mate 禁模组被刷到 60% 差评——UGC 是最强留存引擎，也是中国市场楔子（bilibili/小红书创作者生态），必须抢在克隆前。
> 关联：`docs/strategy/2026-07-07-startup-strategy.md` §4 阶段二。

## 0. 用户决策记录（2026-07-09）

| 岔路口 | 决定 |
|---|---|
| v1 范围 | **本地工坊 MVP**：画廊 + 一键切换 + 导入 + 校验 + 双语创作规范。在线索引、创作预览工具留给后续 PR |
| 身份模型 | **独立角色条目**：每个皮肤是完整 CodexPet 角色文件夹（VPet 模式），零引擎改动；Yoonie 保持默认 + IP 主角地位 |
| 创作门槛 | **宽进严出**：最低一张静态图（自动包成 1 帧 idle），9 行标准图集为推荐格式；导入时完整校验 + 双语友好报错 |
| 实现方案 | **A：模态工坊 + 复用现有管线**（对比过设置页画廊、独立工坊窗口） |

## 1. 现状盘点（设计的地基）

后端管线已存在但前端从未接通：

- **皮肤清单格式 = 现有 `CodexPet` 模型**（`src/lib/utils/codex-pet.ts`）：`pet.json` + 单张精灵图集；无 `animations` 字段则继承 9 行标准图集（`STANDARD_ANIMATION_ROWS`，`DEFAULT_ATLAS` 192×208 8×9）；满配约 25–31 行（yoonie 是参考上限）。
- **自定义目录 `~/.codex/pets`** + `codexpet://` 协议（`src-tauri/src/asset.rs`）+ 四个 Rust 命令（`codex_pet.rs`：list / import / pick folder / open dir）——**全部注册好、零前端调用**。目录布局兼容 codex CLI hatch-pet skill，保持不动（改目录反而砍掉一条现成皮肤来源）。
- **两个断点**导致管线不通：
  1. CSP 未放行 `codexpet:`（macOS）/`http://codexpet.localhost`（Windows）——图集与 pet.json 的加载都会被拦；
  2. 无任何 UI 调用 `list_custom_codex_pets` / `setMiniPetId`——连内置 13 只都没有切换入口。
- 4 处组件（Main / Panel / ProfileCard / ShareCardModal）重复「`loadCodexPets().find(miniPetId) ?? default`」模式，只认内置。
- `SpritePet.svelte` 的 `size` prop = 格子渲染宽度，高度按 `cellH/cellW` 比例——任意尺寸的单图皮肤天然渲染正确。

## 2. 架构

### 2.1 skins 模块（新，薄加载层）

- **`src/lib/utils/skins.ts`**（纯逻辑，vitest 覆盖）：
  - `petJsonUrlFromSheetUrl(sheetUrl)` —— 从 Rust 返回的平台正确 spritesheet URL 推导同目录 `pet.json` URL（复用 Rust 的平台分支，TS 不再判断平台）；
  - `mergeSkins(builtins, customs)` —— id 冲突时**自定义覆盖内置**（与 import 的覆盖升级语义一致）；
  - `tileFrameStyle(pet, tileW)` —— 画廊 tile 的 idle 首帧静态裁切 CSS（background-position/size 数学，与 SpritePet 同源）。
- **`src/lib/stores/skins.svelte.ts`**（新）：
  - `allSkins` 状态：`loadCodexPets()`（内置）+ `list_custom_codex_pets` → 逐只 fetch `pet.json` → 现有 `resolvePet()` 补默认值（需从 codex-pet.ts 导出）→ `mergeSkins`；
  - `revision = $state(0)`：导入/删除成功后 `refresh()` 自增——Main 的选宠 effect 同时追踪 `miniPetId` 与 `revision`，解决「重导入覆盖当前使用中的皮肤但 id 未变」的刷新问题；
  - `resolveSkin(id)`：统一替换 4 处重复 find 逻辑。
- 切换 = `settingsStore.setMiniPetId(id)`（现有持久化 + 响应链）。

### 2.2 Rust 命令（增补）

- `import_codex_pet`：**加固 id 净化**（见 §4 安全），其余复用；
- `import_skin_image(path)`（新）：`imagesize` crate 读尺寸 → 生成 `<slug>/pet.json`（`atlas = {cellW: imgW, cellH: imgH, cols: 1, rows: 1}`，`animations: {idle: {row: 0, frames: 1}}`）+ 复制图片。id/displayName 取文件名 stem（slug 化）；同名覆盖（与文件夹导入一致，规范文档写明）。改名走现有昵称功能；
- `pick_skin_image`（新）：native 文件选择器，过滤 png/webp/jpg/jpeg/gif，复用 `pick_codex_pet_folder` 的 set_parent + reassert_mini_floating 模式；
- `remove_custom_skin(id)`（新）：净化 id 后删除 `~/.codex/pets/<id>`；仅对自定义皮肤暴露 UI。

### 2.3 CSP（tauri.conf.json）

`img-src` 与 `connect-src` 均追加 `codexpet: http://codexpet.localhost`。这是自定义皮肤能加载的前提。

## 3. 导入流程（宽进严出）

```
选文件夹/图片 → Rust 复制进 ~/.codex/pets（同 id 覆盖升级）
→ 前端 fetch 导入结果的 pet.json + 图集尺寸 → skin-validate 校验
→ 致命错误：remove_custom_skin 回滚 + 双语报错列表
→ 通过：refresh() + 自动切换到新皮肤 + （有警告则 tile 角标提示）
```

校验放在复制之后（而非之前）是因为前端无法直接读任意本地路径；回滚保证失败不留脏数据。

## 4. 安全（UGC 威胁模型）

皮肤将来自陌生人分发，导入即写文件系统：

- **路径穿越加固（必须）**：`import_codex_pet` 现状用 pet.json 的 `id` 直接 `root.join(&id)` 作目标目录——`"../../evil"` 可写出 pets 目录外。Rust 侧净化：拒绝含 `/`、`\`、`..`、空串、绝对路径的 id；`remove_custom_skin` 同样净化。TS 校验器同规则（双保险 + 友好报错）。
- 图集/JSON 均静态资源，无代码执行面；`copy_dir_recursive` 只进不出，符号链接最多把内容复制进 pets 目录，无外泄路径。
- 校验器对超大图（>4096×4096）给警告（性能），不拦截。

## 5. 校验器规则（`src/lib/utils/skin-validate.ts`，纯 TS）

输入：解析后的 pet.json（unknown）+ 图集实际尺寸。输出 `{errors, warnings}`，每条 `{key, params}`（i18n key → 双语免费）。

**errors（拦截导入）**：pet.json 非对象 / id 含路径分隔符或 `..` / atlas 四值非正整数 / `cellW×cols > imgW` 或 `cellH×rows > imgH`（越界）/ 声明了 animations 但无 `idle` / 任意动画 `row ≥ rows` / `offsetCol + frames > cols` / `frames < 1`。

**warnings（放行 + 角标提示）**：atlas 未整除图片尺寸（有裁剪余量）/ 缺推荐的 9 行标准动画（列出缺哪些）/ 动画名不在已知词汇表（拼写提示）/ `fps > 60` / 图集超 4096×4096。

## 6. 工坊 UI

- **入口**：Panel 头部行（#46 建立）加 🎨 按钮，两个模式可见，打开 `SkinWorkshopModal`。
- **`SkinWorkshopModal.svelte`**（新，复用 ShareCardModal 模态形态）：
  - 画廊网格：tile = idle 首帧静态裁切（`tileFrameStyle`，不跑动画循环省性能）+ 名字（`effectiveName` 含昵称）+ 使用中高亮 + 自定义角标 + 警告角标；
  - 点 tile = 立即切换；
  - 自定义 tile 的删除按钮（二次确认，调 `remove_custom_skin`）；
  - 底栏：「导入皮肤」（文件夹 / 单张图片双按钮）、「打开皮肤文件夹」（现有 `open_codex_pets_dir`）、「创作指南」（GitHub 上 SKIN-SPEC 链接）。
- 报错呈现：导入失败时模态内列出双语错误项，不用系统弹窗。

## 7. 创作者规范（中国楔子核心交付物）

`docs/skins/`：

- `SKIN-SPEC.zh.md` / `SKIN-SPEC.en.md`：文件夹结构、pet.json 字段参考、9 行标准图集布局表（行号/名称/帧数/fps）、31 行满配进阶表（物理攀爬/键鼠反应/情绪/进食）、校验规则、覆盖升级与命名规则、单图最低门槛说明；
- `template/`：`pet.json`（标准 9 行注释版）+ `README.md`（「把你的 spritesheet.webp 放进来即可导入」）——不含二进制示例图，活体参考指向 `public/assets/builtin/yoonie/`。

这份文档同时是 bilibili/小红书教程的底稿。

## 8. Lore、i18n、遥测

- **Lore canon 第 10 条**（`docs/lore/yoonie.md`）：云上还有很多邻居，Yoonie 会把他们领来家里串门——皮肤 = 云上邻居，贴合独立角色模型。
- **i18n**：`skin.*`（工坊标题/按钮/确认删除）+ `skin.issue.*`（校验错误/警告，带参数插值），en + zh。
- **遥测**（补进 `docs/superpowers/specs/2026-07-08-telemetry-aptabase.md` 字典）：
  - `skin_switched {id}`——内置记原 id，自定义一律记 `custom`（不带用户内容）;
  - `skin_imported {kind: folder|image, result: ok|invalid}`——衡量创作漏斗。

## 9. 测试计划

- vitest：`skin-validate`（每条规则的红绿样例 + 路径穿越样例）、`skins.ts` 纯函数（URL 推导 / merge 覆盖语义 / tile 数学）；
- Rust 命令走手动验收（导入合法皮肤、导入恶意 id、单图导入、删除、重导入覆盖使用中皮肤）；
- 回归：vitest 全量、svelte-check、biome、cargo check。

## 10. 风险与后续

- **内置 IP 风险（Steam 前必须处理）**：内置 13 只含 naruto/nezuko/wukong/doro 等版权角色精灵图，本地分发无人管，但上 Steam 是 DMCA 靶子。工坊即出路：届时把版权角色移出默认包、以社区皮肤形态存在。本 PR 不动内置列表。
- 后续 PR 候选（按序）：在线皮肤索引（社区 GitHub 仓库 + 应用内浏览安装）→ 拖拽导入 → 创作预览工具（拖图实时预览各行动画）。
- 重导入正在使用中的皮肤依赖 `revision` 刷新——如未来有多窗口（demo mascots），它们各自持有 pet 对象，升级刷新留给后续统一。
