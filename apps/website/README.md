# pawbae.ai 官网（apps/website）

C 舞台线领地（见 `docs/team/line-c-stage.md`）。SvelteKit + adapter-static 纯静态导出，Tailwind v4，零 webfont（系统字体栈）。

## 命令

本目录是 pnpm workspace 成员（依赖装在仓库根，无独立 lockfile）：

```bash
pnpm install                  # 在仓库根执行
pnpm --dir apps/website dev   # 本地开发（或在本目录内直接 pnpm dev）
pnpm --dir apps/website build # 静态导出到 build/
pnpm --dir apps/website check # svelte-check
```

## 环境变量

见 `.env.example`。候补名单表单在 `PUBLIC_SUPABASE_URL` / `PUBLIC_SUPABASE_ANON_KEY` 缺失时显示「即将开放」——**公开上线那一刻必须配好真实收集，禁止 no-op 上线**（发布计划红线：静默丢真实报名是最贵的事故）。表单直接 POST Supabase REST `/rest/v1/waitlist`，409（重复邮箱）视为成功。

## 更新清单红线

`static/update/latest.json` 是桌面 App 更新器硬编码的检查地址（`https://pawbae.ai/update/latest.json`，见 `apps/desktop/src-tauri/src/commands/update.rs`）。**任何部署切换前，必须先验证新部署上该文件可达且内容正确**，否则老用户的更新检查全部 404。当前内容与线上（PawBae/website 仓库托管版）一致；Vercel 切到本目录部署时此文件自动接管。

## 部署（cutover 清单）

1. Vercel 项目（现指向 PawBae/website 仓库）→ Settings 改为从本 monorepo 构建：Root Directory = `apps/website`，Framework = SvelteKit，保持默认开启 "Include source files outside of the Root Directory"（pnpm lockfile 在仓库根）；
2. 部署预览环境，验证 `/update/latest.json` 与首页；
3. 配置 `PUBLIC_SUPABASE_*` 环境变量（候补名单真实收集）；
4. Promote 到生产，再次验证清单可达；
5. PawBae/website 老仓库保留只读存档。

## 结构约定

- 视觉 token 全部在 `src/app.css`（骨架严格对照 moneycoach.ai 实站解码）；重设计时改 token 与各组件 `<style>`，不动结构与 Waitlist 逻辑。
- 站点文案后续走 website 独立 i18n 前缀（暂 English-only）。
