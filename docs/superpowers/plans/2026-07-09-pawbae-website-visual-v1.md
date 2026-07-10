# PawBae Website Visual v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing `apps/website/` scaffold into a production-ready PawBae marketing site using the approved “Bobo on the terminal window” visual direction.

**Architecture:** Keep the single SvelteKit route and existing section order. Centralize global tokens and shared primitives in `src/app.css`, keep page-specific art and layout inside `+page.svelte`, and keep navigation/footer styling inside `+layout.svelte`. Inline SVG is used for the custom mascot and terminal scene so the static export has no external assets or runtime dependencies.

**Tech Stack:** Svelte 5, SvelteKit 2, adapter-static, Tailwind v4, native CSS, inline SVG, pnpm.

## Global Constraints

- Work only on `design/website-v1`; never push directly to `main` and do not create a PR.
- Do not add runtime dependencies, webfonts, CDNs, or external image URLs.
- Preserve section order and ids: `download`, `features`, `visits`, `privacy`, `details`, `faq`, `waitlist`.
- Preserve `apps/website/src/routes/+layout.ts`, adapter-static configuration, and `apps/website/static/update/latest.json` byte-for-byte.
- Preserve the env/fetch/status logic in the first 34 lines of `src/lib/Waitlist.svelte`.
- Use the system font stack and the exact primary colors `#0a0a08`, `#0562C0`, and `#facc15`.
- Use 12px button radii, 16px card radii, and full-radius pills/status dots.
- All motion must respect `prefers-reduced-motion`.
- Body text must meet WCAG AA; all focus states must be visible.

---

### Task 1: Freeze protected boundaries and establish the global visual system

**Files:**
- Modify: `apps/website/src/app.css`
- Verify unchanged: `apps/website/static/update/latest.json`
- Verify logic-preserved: `apps/website/src/lib/Waitlist.svelte:1-34`

**Interfaces:**
- Consumes: Existing shared classes `.wrap`, `.kicker`, `.h2`, `.lede`, `.btn`, `.dl`, `.glass-card`, `.sdot`.
- Produces: Stable color, type, spacing, radius, focus, and motion tokens consumed by all later tasks.

- [ ] **Step 1: Record protected hashes before editing**

Run:

```bash
shasum -a 256 static/update/latest.json
sed -n '1,34p' src/lib/Waitlist.svelte | shasum -a 256
```

Expected:

```text
e69d3db9433e7b010663d74d48ea9d603027a43434d1d28924d85a0205350493  static/update/latest.json
f6c52c1271aec0355c1436bf26338d20c56c478ef4d435178de2e9cab3fe204d  -
```

- [ ] **Step 2: Replace the token block and shared primitives**

Implement these exact roles in `src/app.css`:

```css
:root {
  --hero-bg: #0a0a08;
  --band-950: #030712;
  --band-900: #111827;
  --primary: #0562c0;
  --primary-hover: #064f99;
  --yellow: #facc15;
  --ink-950: #020617;
  --ink-900: #111827;
  --ink-600: #475569;
  --ink-500: #64748b;
  --ink-400: #94a3b8;
  --surface: #ffffff;
  --surface-soft: #f8fafc;
  --line: rgba(226, 232, 240, 0.84);
  --mint: #34d399;
  --peach: #fb923c;
  --lavender: #a78bfa;
  --sky: #7dd3fc;
  --radius-button: 12px;
  --radius-card: 16px;
  --section-space: clamp(5rem, 9vw, 8rem);
  --font-sans: -apple-system, BlinkMacSystemFont, 'SF Pro Display', 'Segoe UI', 'PingFang SC', sans-serif;
  --font-mono: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, Consolas, monospace;
}

html {
  scroll-behavior: smooth;
  overflow-x: clip;
}

body {
  margin: 0;
  min-width: 320px;
  background: var(--surface);
  color: var(--ink-600);
  font-family: var(--font-sans);
  line-height: 1.65;
  -webkit-font-smoothing: antialiased;
}

:focus-visible {
  outline: 3px solid color-mix(in srgb, var(--primary) 78%, white);
  outline-offset: 3px;
}

@media (prefers-reduced-motion: reduce) {
  html { scroll-behavior: auto; }
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 3: Verify CSS parses through SvelteKit**

Run: `pnpm check`

Expected: exit 0 with `0 errors and 0 warnings`.

- [ ] **Step 4: Commit the visual foundation**

```bash
git add apps/website/src/app.css
git commit -m "design(website): establish visual system"
```

### Task 2: Redesign navigation, footer, and brand mark

**Files:**
- Modify: `apps/website/src/routes/+layout.svelte`
- Modify: `apps/website/static/favicon.svg`

**Interfaces:**
- Consumes: `.wrap`, `.btn`, `.btn-primary`, token roles from Task 1.
- Produces: `site-nav`, `brand-mark`, `footer-grid`, and a unified paw/Bobo brand silhouette.

- [ ] **Step 1: Add semantic navigation and the new brand mark**

Use this structure, preserving link targets:

```svelte
<a class="skip-link" href="#download">Skip to content</a>
<nav class="site-nav" aria-label="Primary navigation">
  <div class="wrap nav-inner">
    <a class="logo" href="/" aria-label="PawBae home">
      <svg class="brand-mark" viewBox="0 0 36 36" aria-hidden="true">
        <rect x="2" y="2" width="32" height="32" rx="10" fill="#0a0a08" />
        <path d="M10 16 9 8l7 5m10 3 1-8-7 5" fill="#7dd3fc" />
        <ellipse cx="18" cy="21" rx="11" ry="9" fill="#ffe3c2" />
        <path d="M27 24c5 0 5-4 3-6" fill="none" stroke="#fb923c" stroke-width="3" stroke-linecap="round" />
        <circle cx="14" cy="20" r="1.4" fill="#734b2e" />
        <circle cx="22" cy="20" r="1.4" fill="#734b2e" />
      </svg>
      <span>PawBae</span>
    </a>
    <div class="nav-links">
      <a href="/#features">Features</a>
      <a href="/#visits">Visits</a>
      <a href="/#privacy">Privacy</a>
      <a href="/#faq">FAQ</a>
    </div>
    <a class="btn btn-primary nav-cta" href="/#download">Download Free</a>
  </div>
</nav>
```

The inline SVG must contain a rounded cream face, blue ear tips, peach tail accent, and no text paths.

- [ ] **Step 2: Rebuild footer spacing without changing information architecture**

Use exactly six desktop columns, three at `max-width: 980px`, and two at `max-width: 620px`:

```css
.footer-grid {
  display: grid;
  grid-template-columns: minmax(180px, 1.4fr) repeat(5, minmax(0, 1fr));
  gap: 2.5rem 1.5rem;
}
@media (max-width: 980px) { .footer-grid { grid-template-columns: repeat(3, 1fr); } }
@media (max-width: 620px) { .footer-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
```

- [ ] **Step 3: Replace favicon with the same simplified mascot mark**

The favicon must remain a standalone SVG with `viewBox="0 0 36 36"`, a dark rounded-square base, cream face, blue ears, and peach tail accent.

- [ ] **Step 4: Verify keyboard focus and layout compilation**

Run: `pnpm check`

Expected: exit 0, no Svelte accessibility warnings.

- [ ] **Step 5: Commit site chrome**

```bash
git add apps/website/src/routes/+layout.svelte apps/website/static/favicon.svg
git commit -m "design(website): refine navigation and footer"
```

### Task 3: Build the Bobo-on-terminal Hero and tools band

**Files:**
- Modify: `apps/website/src/routes/+page.svelte:1-120`
- Modify: `apps/website/src/routes/+page.svelte:<style>`

**Interfaces:**
- Consumes: shared button and typography classes from Task 1.
- Produces: `.hero`, `.hero-copy`, `.hero-stage`, `.terminal-window`, `.bobo-perch`, `.agent-status`, `.tools-band`.

- [ ] **Step 1: Replace Hero copy stack with the approved hierarchy**

Use this exact content order:

```svelte
<header class="hero" id="download">
  <div class="wrap hero-inner">
    <div class="hero-copy">
      <a class="ship-pill" href="https://github.com/PawBae/PawBae-app/blob/main/CHANGELOG.md">
        <span class="status-dot working"></span>
        <span>Just shipped v0.2.0</span>
        <span class="pill-link">Read the changelog</span>
      </a>
      <h1>Your agent. Your pet. Your desktop.</h1>
      <p class="hero-lede">PawBae turns Claude Code, Codex, and Cursor activity into a companion you can read at a glance.</p>
      <div class="hero-actions">
        <a class="btn btn-yellow" href="https://github.com/PawBae/PawBae-app/releases/latest">Download Now For Free</a>
        <a class="btn btn-ghost-dark" href="#features">See how it works</a>
      </div>
    </div>
    <div class="hero-stage">
      <div class="terminal-window" aria-label="Claude Code fixing a flaky authentication test">
        <div class="terminal-bar">
          <span class="window-dot close"></span>
          <span class="window-dot minimize"></span>
          <span class="window-dot maximize"></span>
          <span class="terminal-title">claude - ~/projects/pawbae</span>
        </div>
        <div class="terminal-body" aria-hidden="true">
          <p><span class="prompt">❯</span> claude "trace the flaky auth test"</p>
          <p><span class="tool">●</span> Read(src/routes/auth.test.ts)</p>
          <p class="terminal-line secondary"><span class="tool">●</span> Edit(src/lib/session.ts)</p>
          <p class="terminal-line secondary"><span class="tool">●</span> Bash(pnpm test auth)</p>
          <p><span class="success">✓</span> 18 tests passed</p>
          <p class="done">Done. The race condition is fixed.</p>
        </div>
      </div>
      <svg class="bobo-perch" viewBox="0 0 300 190" role="img" aria-label="Bobo the PawBae pet resting on the terminal window">
        <path d="M235 124c46 5 58-23 39-45" fill="none" stroke="#fb923c" stroke-width="22" stroke-linecap="round" />
        <path d="m84 63-13-43 47 29m98 14 13-43-47 29" fill="#7dd3fc" stroke="#efc28f" stroke-width="7" stroke-linejoin="round" />
        <ellipse cx="150" cy="92" rx="79" ry="64" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
        <path d="M112 89q12 14 24 0m28 0q12 14 24 0" fill="none" stroke="#734b2e" stroke-width="7" stroke-linecap="round" />
        <path d="m145 111 5 4 5-4m-5 4v8" fill="none" stroke="#734b2e" stroke-width="5" stroke-linecap="round" />
        <ellipse cx="98" cy="111" rx="12" ry="7" fill="#f9a889" opacity=".65" />
        <ellipse cx="202" cy="111" rx="12" ry="7" fill="#f9a889" opacity=".65" />
        <rect x="100" y="137" width="39" height="43" rx="19" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
        <rect x="161" y="137" width="39" height="43" rx="19" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
      </svg>
      <span class="agent-status"><span class="status-dot working"></span>Claude Code is working</span>
    </div>
  </div>
</header>
```

- [ ] **Step 2: Draw a credible terminal window and Bobo SVG**

The terminal must show these lines as real text inside the SVG/HTML stage:

```text
❯ claude "trace the flaky auth test"
● Read(src/routes/auth.test.ts)
● Edit(src/lib/session.ts)
● Bash(pnpm test auth)
✓ 18 tests passed
Done. The race condition is fixed.
```

Bobo must overlap the terminal title bar, show two front paws over the frame, retain both ears and the full tail at 375px, and expose `role="img"` with an accessible label on the containing SVG.

- [ ] **Step 3: Implement responsive art direction**

Use desktop H1 at 72px max and mobile at 40px, with explicit rules:

```css
.hero h1 { font-size: clamp(2.5rem, 6.4vw, 4.5rem); letter-spacing: -0.035em; line-height: 1.02; }
.hero-stage { margin-top: clamp(3rem, 7vw, 5.5rem); }
@media (max-width: 640px) {
  .hero { padding-top: 4.5rem; }
  .hero-actions { align-items: stretch; flex-direction: column; }
  .hero-actions .btn { width: 100%; text-align: center; }
  .terminal-body { min-height: 248px; overflow: hidden; }
  .terminal-line.secondary { display: none; }
}
```

- [ ] **Step 4: Rebuild the tools band**

Use a concise centered heading, one local-processing sentence, and three consistent tool pills labeled `Claude Code`, `Codex`, and `Cursor`.

- [ ] **Step 5: Verify Hero and commit**

Run: `pnpm check && pnpm build`

Expected: both commands exit 0 and adapter-static writes `build/index.html`.

```bash
git add apps/website/src/routes/+page.svelte
git commit -m "design(website): create Bobo terminal hero"
```

### Task 4: Unify feature, visits, privacy, and details art direction

**Files:**
- Modify: `apps/website/src/routes/+page.svelte:features-visits-privacy-details`

**Interfaces:**
- Consumes: Bobo shape language and state colors from Tasks 1 and 3.
- Produces: `.state-stage`, `.friends-panel`, `.memory-card`, `.privacy-house`, `.details-grid`.

- [ ] **Step 1: Replace emoji feature bullets with consistent inline icons**

Each `.dl .row` uses a small inline SVG or CSS shape positioned in `.ic`, followed immediately by bold state copy:

```svelte
<div class="row">
  <span class="ic ic-mint" aria-hidden="true">
    <svg viewBox="0 0 24 24">
      <path d="M5 8h14v10H5zM8 5h8v3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
      <path d="m9 13 2 2 4-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </span>
  <b>Working.</b> Bobo pitches a tiny tent while long agent tasks run.
</div>
```

Repeat with `Waiting on you.` and `Done. Really done.`. Do not create three equal cards.

- [ ] **Step 2: Build the semantic state stage**

Render four rows in this order: working/mint, waiting/peach, compacting/lavender, offline/slate. Include one shared Bobo illustration at the bottom rather than a separate mascot per row.

- [ ] **Step 3: Build visits friend list and memory card**

Preserve three friends and statuses. Replace emoji avatars with three small mascot face SVGs. The memory card title remains `Shipped it. Together.` and the timestamp remains short and non-interactive.

- [ ] **Step 4: Redraw privacy and details visuals**

The privacy art uses a house outline, local-device window, shield/lock geometry, and Bobo at the threshold. The details area uses a 2×2 grid with two stronger visual cells and two lighter cells, with titles `OBS stream stage`, `Pet diary`, `Eggs & dex`, and `Skin workshop`.

- [ ] **Step 5: Verify structure and commit**

Run:

```bash
rg -n 'id="(download|features|visits|privacy|details|faq|waitlist)"' src/routes/+page.svelte
pnpm check
```

Expected: exactly seven id matches in the original order; check exits 0.

```bash
git add apps/website/src/routes/+page.svelte
git commit -m "design(website): unify feature illustration system"
```

### Task 5: Polish FAQ and Waitlist without altering submission behavior

**Files:**
- Modify: `apps/website/src/routes/+page.svelte:faq-waitlist`
- Modify style/markup only: `apps/website/src/lib/Waitlist.svelte:36-end`

**Interfaces:**
- Consumes: form, button, focus, and dark-band tokens from Task 1.
- Produces: keyboard-safe FAQ disclosures and complete idle/sending/done/error/unconfigured waitlist presentation.

- [ ] **Step 1: Use native disclosure behavior and a CSS plus indicator**

Keep native disclosure markup. For example, the first item remains exactly:

```svelte
<details>
  <summary>Which coding agents does PawBae support?</summary>
  <p>Claude Code, Codex and Cursor today, via their local hook events. More connectors are on the roadmap. The pet works fine without an agent, too.</p>
</details>
```

Use a two-line pseudo-element for the indicator so open state removes the vertical line instead of inserting an en dash character.

```css
.faq summary::before,
.faq summary::after {
  content: '';
  position: absolute;
  right: 0.25rem;
  top: 50%;
  width: 0.85rem;
  height: 2px;
  background: currentColor;
}
.faq summary::before { transform: translateY(-50%) rotate(90deg); }
.faq details[open] summary::before { transform: translateY(-50%) rotate(0); opacity: 0; }
```

- [ ] **Step 2: Restyle the Waitlist states below line 34 only**

Add a visible `<label class="sr-only" for="waitlist-email">Email address</label>`, add `id="waitlist-email"`, and retain the same `bind:value`, `required`, `onsubmit={submit}`, disabled condition, status branches, and button expressions.

- [ ] **Step 3: Prove Waitlist logic is unchanged**

Run:

```bash
sed -n '1,34p' src/lib/Waitlist.svelte | shasum -a 256
```

Expected:

```text
f6c52c1271aec0355c1436bf26338d20c56c478ef4d435178de2e9cab3fe204d  -
```

- [ ] **Step 4: Verify and commit**

Run: `pnpm check`

Expected: `0 errors and 0 warnings`.

```bash
git add apps/website/src/routes/+page.svelte apps/website/src/lib/Waitlist.svelte
git commit -m "design(website): polish FAQ and waitlist states"
```

### Task 6: Responsive, accessibility, build, and protected-file verification

**Files:**
- Modify only if findings require: `apps/website/src/app.css`, `apps/website/src/routes/+layout.svelte`, `apps/website/src/routes/+page.svelte`, `apps/website/src/lib/Waitlist.svelte`
- Create screenshot artifacts outside git: `/tmp/pawbae-375.png`, `/tmp/pawbae-768.png`, `/tmp/pawbae-1280.png`, `/tmp/pawbae-1440.png`

**Interfaces:**
- Consumes: completed page.
- Produces: verified static build, four viewport screenshots, and a clean branch ready to push.

- [ ] **Step 1: Install and run the complete verification commands**

Run:

```bash
pnpm install
pnpm build
pnpm check
```

Expected: all commands exit 0; no Svelte warnings; `build/index.html` exists.

- [ ] **Step 2: Run four viewport checks**

At 375×812, 768×900, 1280×900, and 1440×1000, capture screenshots and evaluate:

```js
({
  width: innerWidth,
  clientWidth: document.documentElement.clientWidth,
  scrollWidth: document.documentElement.scrollWidth,
  overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
})
```

Expected at every width: `overflow: false` and `scrollWidth === clientWidth`.

- [ ] **Step 3: Perform accessibility checks**

Verify keyboard Tab order reaches skip link, nav links, Hero CTAs, FAQ summaries, waitlist input/button, and footer links. Verify Enter/Space toggles FAQ details. Emulate reduced motion and confirm no looping movement remains.

- [ ] **Step 4: Compare the final page to MoneyCoach rhythm**

Check: navigation ≤72px, Hero H1 max 72px, primary buttons 48px high with 12px radius, light H2 around 36/40px, 96-128px desktop section spacing, alternating dark/light/color bands, and six-column white footer.

- [ ] **Step 5: Recheck protected files and working tree**

Run:

```bash
shasum -a 256 static/update/latest.json
sed -n '1,34p' src/lib/Waitlist.svelte | shasum -a 256
git diff --check
git status --short
```

Expected hashes match Task 1. Only intentional website visual files may be modified; `.codex/` remains untouched and untracked.

- [ ] **Step 6: Commit final polish and push without a PR**

```bash
git add apps/website/src/app.css apps/website/src/routes/+layout.svelte apps/website/src/routes/+page.svelte apps/website/src/lib/Waitlist.svelte apps/website/static/favicon.svg
git commit -m "design(website): finalize responsive PawBae landing page"
git push -u origin design/website-v1
```

Expected: branch pushes successfully; do not run `gh pr create`.
