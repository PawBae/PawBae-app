# Solu · 小煦（v1 beta 官方宠）

日元素 · 花瓣鬃毛 · 热情阳光。设计定稿见
`docs/superpowers/specs/2026-07-10-pet-interaction-creation-design.md` §7。

本目录包含：

- `pet.json` — 4 × 4、单格 256 × 256 的精灵包元数据；
- `spritesheet.png` — 1024 × 1024 透明 PNG，覆盖 idle、running、working、
  waiting、happy、sleep、arrival 与 return；
- `README.md` — 资产范围和验收入口。

Solu 已进入 `pets-manifest.json` 的可加载列表，也是 v1 onboarding 唯一开放选择的
新官方宠物。其他尚未完成的官方宠物继续保留在 `upcoming`。

提交前运行 `pnpm pet-assets:check`，校验 manifest、图集尺寸、必需状态和帧范围。
