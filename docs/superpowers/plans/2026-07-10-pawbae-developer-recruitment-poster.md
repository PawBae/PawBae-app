# PawBae Developer Recruitment Poster Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate a polished 4:5 Chinese recruitment poster for Xiaohongshu that recruits two PawBae developers for July–September 2026.

**Architecture:** Use the built-in image generation workflow with the existing PawBae app icon as the only visual reference. Generate one final raster poster using the approved orange-red and navy art direction, then visually verify brand fidelity, exact copy, hierarchy, and mobile readability before retaining the selected output.

**Tech Stack:** Built-in ImageGen, PawBae PNG brand asset, Markdown design specification

## Global Constraints

- Channel and format: Xiaohongshu portrait poster, 4:5 aspect ratio.
- Recruit exactly 2 developers for July–September 2026.
- Use `src-tauri/icons/icon.png` as the PawBae brand and mascot reference.
- Preserve the approved orange-red, navy, warm-yellow, and white visual direction.
- CTA must be exactly `感兴趣？直接私信我`.
- Do not add salary, location, work-hour, QR-code, watermark, or contact claims.
- Keep `免费 AI Coding 账号支持`; actual account provisioning must use independent, compliant authorization.

---

### Task 1: Generate the approved poster

**Files:**
- Reference: `src-tauri/icons/icon.png`
- Reference: `docs/superpowers/specs/2026-07-10-pawbae-developer-recruitment-poster-design.md`
- Create: built-in ImageGen poster output

**Interfaces:**
- Consumes: approved visual direction, exact poster copy, and PawBae icon reference
- Produces: one 4:5 portrait recruitment poster raster

- [ ] **Step 1: Supply the PawBae icon as the brand-reference image**

Use `/Users/user/Documents/UW/Projects/PawBae-app/src-tauri/icons/icon.png` as a reference image, not an edit target. Preserve the white rounded pet, heart-shaped belly mark, lavender outline, and friendly facial character.

- [ ] **Step 2: Generate the poster with the approved prompt**

```text
Use case: ads-marketing
Asset type: Xiaohongshu recruitment poster, portrait 4:5
Primary request: Create a high-impact Chinese recruitment poster for PawBae, recruiting exactly two developers for July through September 2026.
Input image: PawBae app icon, used as the exact brand and mascot reference.
Scene/backdrop: bold diagonal color-block composition using vivid orange-red in the upper-left and deep navy in the lower-right, with small warm-yellow accents and ample clean white space around text.
Subject: the recognizable PawBae white rounded desktop-pet mascot with lavender outline and heart-shaped belly mark, placed as a strong visual anchor in the lower-right without covering any copy.
Style/medium: polished contemporary product recruitment campaign; energetic, youthful, graphic, editorial, Xiaohongshu-native; clean 2D poster design rather than a corporate job ad.
Composition/framing: 4:5 vertical. Small eyebrow at top, huge Chinese headline in the upper half, short project explanation, two compact information panels, tech-stack line, then a prominent CTA at the bottom. Make “招募开发搭子” and “仅招 2 人” readable at thumbnail size.
Lighting/mood: bright, energetic, optimistic, collaborative.
Color palette: orange-red, deep navy, warm yellow, crisp white; retain the mascot’s lavender, peach, mint, and yellow details.
Typography: bold modern simplified-Chinese sans serif for the headline, highly legible compact sans serif for body copy, disciplined spacing and strong contrast.
Text (verbatim):
“PAWBAE SUMMER TEAM · 仅招 2 人”
“招募开发搭子”
“一起做一只会陪你写代码的桌面宠物”
“PawBae 是一款 AI 驱动的桌面宠物 App，能够实时感知 Codex、Claude Code、Cursor 的工作状态，在编程时陪伴你、回应你。”
“你将获得”
“免费 AI Coding 账号支持”
“真实跨平台 App 项目经历”
“参与产品设计、开发与迭代”
“我们希望你”
“有热情，对 PawBae 感兴趣”
“有 App 开发经验”
“2026 年 7–9 月有稳定时间”
“Svelte 5 · TypeScript · Tauri 2 · Rust”
“感兴趣？直接私信我”
Constraints: render every supplied line exactly once; all Chinese must be correct simplified Chinese; preserve all English capitalization and product names; keep mascot fully visible and faithful to the reference; clear information hierarchy; mobile-readable.
Avoid: QR codes, salaries, location claims, work-hour claims, extra contact details, photorealism, generic office imagery, extra mascots, text duplication, gibberish characters, warped logo, watermark.
```

- [ ] **Step 3: Retain the built-in generation result for review**

Expected: one rendered poster visible in the Codex conversation with the PawBae mascot and the approved recruitment copy.

### Task 2: Verify the final poster

**Files:**
- Inspect: built-in ImageGen poster output

**Interfaces:**
- Consumes: Task 1 poster output
- Produces: acceptance or one narrowly scoped regeneration request

- [ ] **Step 1: Check brand fidelity**

Expected: the mascot remains a white rounded pet with a lavender outline, friendly face, and heart-shaped belly mark. It must not be cropped, obscured, or materially redesigned.

- [ ] **Step 2: Check content accuracy**

Expected: the poster says there are 2 openings, the dates are 2026 年 7–9 月, the CTA is `感兴趣？直接私信我`, and no unapproved salary, location, hours, QR code, or contact detail appears.

- [ ] **Step 3: Check mobile hierarchy**

Expected: `招募开发搭子` is the dominant headline; `仅招 2 人` remains visible at thumbnail scale; benefits, requirements, stack, and CTA read in a clear top-to-bottom sequence.

- [ ] **Step 4: Regenerate only if a blocking defect is visible**

If any supplied line is misspelled or unreadable, issue one targeted regeneration preserving the approved composition, palette, mascot, and all correctly rendered text while correcting only the defective copy.
