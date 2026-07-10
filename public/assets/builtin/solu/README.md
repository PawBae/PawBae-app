# Solu · 小煦(官方宠 · 资产制作中)

日元素 · 花瓣鬃毛 · 热情阳光。设计定稿见
`docs/superpowers/specs/2026-07-10-pet-interaction-creation-design.md` §7。

资产就绪后本目录放置:

- `pet.json` — 精灵包元数据(格式见 `docs/skins/SKIN-SPEC.zh.md`;cellW 384~512)
- `spritesheet.webp` — 图集(羊毛毡定格动画风,帧数预算 idle 1~2 / running 4 / happy 2~3 / eat 2 / failed 2 / working 2~3)

同时把 id `solu` 从 `pets-manifest.json` 的 `upcoming` 挪入 `pets`(加载器只读 `pets`)。
