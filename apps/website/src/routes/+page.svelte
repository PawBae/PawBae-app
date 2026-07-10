<script lang="ts">
  import Waitlist from '$lib/Waitlist.svelte';
</script>

<!-- 结构骨架严格对照 moneycoach.ai 实站条带节奏；视觉层待重设计，改样式不动结构 -->

<header class="hero" id="download">
  <div class="wrap hero-in">
    <a class="ship-pill" href="https://github.com/PawBae/PawBae-app/blob/main/CHANGELOG.md">
      <span class="dot"></span> Just shipped v0.2.0 — OBS stream stage · Changelog →
    </a>
    <h1>The desktop pet that watches your AI agents</h1>
    <p class="sub">
      Your agent. Your pet. Your desktop. PawBae works when Claude Code works, waits when it waits,
      and celebrates when you ship.
    </p>
    <div class="ctas">
      <a class="btn btn-yellow" href="https://github.com/PawBae/PawBae-app/releases/latest">Download Now For Free</a>
      <a class="btn btn-ghost-dark" href="#waitlist">Join the waitlist</a>
    </div>
    <p class="cta-note">
      macOS (Apple silicon) · Windows beta · Closed beta, <a href="#waitlist">invite only</a>
    </p>

    <div class="hero-shot">
      <div class="hero-glow"></div>
      <div class="desk">
        <div class="pet-perch">
          <svg viewBox="0 0 120 100" aria-label="PawBae pet sitting on the terminal">
            <path class="pet-tail" d="M92 70 q18 0 15 -16" stroke="#f5b06c" stroke-width="9" fill="none" stroke-linecap="round" />
            <path d="M36 42 L29 20 L52 31 Z" fill="#ffd9ae" stroke="#f5b06c" stroke-width="3" stroke-linejoin="round" />
            <path d="M84 42 L91 20 L68 31 Z" fill="#ffd9ae" stroke="#f5b06c" stroke-width="3" stroke-linejoin="round" />
            <ellipse cx="60" cy="62" rx="36" ry="31" fill="#ffe3c2" />
            <path d="M45 58 q5 6 10 0" stroke="#8a5a2b" stroke-width="3" fill="none" stroke-linecap="round" />
            <path d="M65 58 q5 6 10 0" stroke="#8a5a2b" stroke-width="3" fill="none" stroke-linecap="round" />
            <path d="M55 70 q5 4 10 0" stroke="#8a5a2b" stroke-width="2.5" fill="none" stroke-linecap="round" />
            <ellipse cx="38" cy="68" rx="6" ry="4" fill="#ffb98a" opacity="0.7" />
            <ellipse cx="82" cy="68" rx="6" ry="4" fill="#ffb98a" opacity="0.7" />
          </svg>
        </div>
        <span class="status-chip"><span class="dot"></span> working…</span>
        <div class="hero-toast">Momo dropped by 👋</div>
        <div class="term">
          <div class="term-bar">
            <span class="tl" style="background:#ff5f57"></span><span class="tl" style="background:#febc2e"></span><span class="tl" style="background:#28c840"></span>
            <span class="term-title">claude — ~/projects/orbit-api</span>
          </div>
          <div class="term-body">
            <div><span class="pr">❯</span> claude "add rate limiting to the auth routes"</div>
            <div><span class="ok">⏺</span> Read(src/routes/auth.ts)</div>
            <div><span class="ok">⏺</span> Edit(src/middleware/rateLimit.ts)</div>
            <div><span class="dim">✻ Running tests… 42 passed</span></div>
            <div><span class="ok">⏺</span> Done. Rate limiting added to 3 routes.</div>
          </div>
        </div>
      </div>
    </div>
  </div>
</header>

<section class="tools-band">
  <div class="wrap">
    <h2 class="h2 on-dark">Works with the tools you live in</h2>
    <p class="sub2">Local hook events in, presence out. No screenshots, no scraping, no telemetry of your code.</p>
    <div class="tool-row">
      <span class="tool-mark">Claude Code</span>
      <span class="tool-mark">Codex</span>
      <span class="tool-mark">Cursor</span>
    </div>
  </div>
</section>

<section class="feature" id="features">
  <div class="wrap split">
    <div>
      <p class="kicker">Presence, not dashboards</p>
      <h2 class="h2">It knows what your agent is doing</h2>
      <p class="lede">No status windows to check. Your pet <em>is</em> the status — readable from across the room.</p>
      <div class="dl">
        <div class="row"><span class="ic" style="background:#ecfdf5">⛺</span><b>Working.</b> A tiny tent goes up while long tasks run. Glance over, see it's still busy, get back to your coffee.</div>
        <div class="row"><span class="ic" style="background:#fff7ed">📝</span><b>Waiting on you.</b> When your agent needs approval, your pet picks up a note and looks at you. No more missed prompts.</div>
        <div class="row"><span class="ic" style="background:#eff6ff">🎉</span><b>Done — really done.</b> A soft chirp and a little celebration, only on genuine completion. Sub-agents stay quiet.</div>
      </div>
    </div>
    <div class="state-card" aria-label="pet state examples">
      <div class="state-row"><span class="sdot" style="background:var(--mint)"></span><b>working</b> pitches a tent for the long runs</div>
      <div class="state-row"><span class="sdot" style="background:var(--peach)"></span><b>waiting</b> holds up a note until you approve</div>
      <div class="state-row"><span class="sdot" style="background:var(--lav)"></span><b>compacting</b> tidies its little clouds</div>
      <div class="state-row"><span class="sdot" style="background:#9ca3af"></span><b>offline</b> curls up and naps</div>
      <div class="state-pet">
        <svg viewBox="0 0 120 100" width="96" aria-hidden="true">
          <path d="M36 42 L29 20 L52 31 Z" fill="#ffd9ae" /><path d="M84 42 L91 20 L68 31 Z" fill="#ffd9ae" />
          <ellipse cx="60" cy="62" rx="36" ry="31" fill="#ffe3c2" />
          <circle cx="50" cy="58" r="3.4" fill="#8a5a2b" /><circle cx="70" cy="58" r="3.4" fill="#8a5a2b" />
          <path d="M55 70 q5 4 10 0" stroke="#8a5a2b" stroke-width="2.5" fill="none" stroke-linecap="round" />
        </svg>
      </div>
    </div>
  </div>
</section>

<section class="visit-band" id="visits">
  <div class="wrap split">
    <div>
      <p class="kicker">New in the closed beta</p>
      <h2 class="h2">Friends' pets come to visit</h2>
      <p class="lede">
        Send your pet to a friend's desktop for a 30-minute visit. Two pets, two agents, one desk —
        and a shared memory when it comes home.
      </p>
      <div class="dl">
        <div class="row"><span class="ic" style="background:#eff6ff">🎒</span><b>Send your pet over.</b> Pick a friend, they accept, your pet packs a tiny bag. Recall it any time, no penalty.</div>
        <div class="row"><span class="ic" style="background:#fff7ed">🐾</span><b>Two pets, one desk.</b> They boop noses, sit side by side, chase paper planes — each still mirroring its own human's agent.</div>
        <div class="row"><span class="ic" style="background:#f5f3ff">📸</span><b>Come home with a memory.</b> Every visit ends as a keepsake card in both diaries. No points, no streaks — just the moment.</div>
      </div>
    </div>
    <div>
      <div class="glass-card">
        <div class="friend-row">
          <span class="avatar" style="background:#e0f2fe">🐱</span>
          <span class="friend-meta"><b>Momo</b><span>@keyu · friends since May</span></span>
          <span class="friend-state"><span class="sdot" style="background:var(--mint)"></span>working</span>
        </div>
        <div class="friend-row">
          <span class="avatar" style="background:#fff7ed">🐹</span>
          <span class="friend-meta"><b>Tofu</b><span>@sarahk · friends since June</span></span>
          <span class="friend-state"><span class="sdot" style="background:var(--peach)"></span>idle</span>
        </div>
        <div class="friend-row">
          <span class="avatar" style="background:#f3f4f6">🐰</span>
          <span class="friend-meta"><b>Mochi</b><span>@devon · friends since June</span></span>
          <span class="friend-state"><span class="sdot" style="background:#9ca3af"></span>offline</span>
        </div>
      </div>
      <div class="glass-card memory-card">
        <div class="memory-art">
          <svg viewBox="0 0 320 150" width="100%" height="100%" aria-label="two pets celebrating">
            <ellipse cx="115" cy="104" rx="32" ry="27" fill="#ffe3c2" />
            <path d="M95 84 L89 66 L109 76 Z" fill="#ffd9ae" /><path d="M135 84 L141 66 L121 76 Z" fill="#ffd9ae" />
            <path d="M104 100 q4 5 9 0 M121 100 q4 5 9 0" stroke="#8a5a2b" stroke-width="2.5" fill="none" stroke-linecap="round" />
            <ellipse cx="205" cy="106" rx="30" ry="25" fill="#d6edff" />
            <path d="M186 88 L180 70 L200 80 Z" fill="#bfe3ff" /><path d="M224 88 L230 70 L210 80 Z" fill="#bfe3ff" />
            <path d="M195 102 q4 5 9 0 M212 102 q4 5 9 0" stroke="#2b4a6f" stroke-width="2.5" fill="none" stroke-linecap="round" />
            <text x="150" y="58" font-size="20">🎉</text>
          </svg>
        </div>
        <div class="memory-body">
          <h3>Shipped it. Together.</h3>
          <time>Bobo × Momo · Tue evening · 28 min</time>
        </div>
      </div>
    </div>
  </div>
</section>

<section class="feature on-dark-sec" id="privacy">
  <div class="wrap split">
    <div>
      <p class="kicker sky-kicker">Privacy</p>
      <h2 class="h2 on-dark">Local. Private. Yours.</h2>
      <p class="lede">The pet reads your agent so you don't have to — and nobody else gets to.</p>
      <div class="dl">
        <div class="row"><span class="ic dark-ic">💻</span><b>On-device by default.</b> Hook events are processed locally. Your prompts, code and file paths never leave your machine.</div>
        <div class="row"><span class="ic dark-ic">🌫️</span><b>A vibe, not your code.</b> Friends see five moods — idle, working, waiting, tidying, offline. Never task text, never which tool, never your repo.</div>
        <div class="row"><span class="ic dark-ic">🔒</span><b>You hold the leash.</b> Visits are invite-only between mutual friends. Decline, recall, mute or block — no penalties, ever.</div>
      </div>
    </div>
    <div class="privacy-art">
      <svg viewBox="0 0 200 160" width="230" aria-label="pet guarding a locked house">
        <rect x="40" y="58" width="120" height="76" rx="12" fill="none" stroke="rgba(255,255,255,.25)" stroke-width="3" />
        <path d="M32 62 L100 22 L168 62" fill="none" stroke="rgba(255,255,255,.25)" stroke-width="3" stroke-linecap="round" />
        <ellipse cx="100" cy="112" rx="28" ry="23" fill="#ffe3c2" />
        <path d="M82 94 L76 76 L96 86 Z" fill="#ffd9ae" /><path d="M118 94 L124 76 L104 86 Z" fill="#ffd9ae" />
        <circle cx="92" cy="108" r="3" fill="#8a5a2b" /><circle cx="108" cy="108" r="3" fill="#8a5a2b" />
        <text x="140" y="90" font-size="16">🔒</text>
      </svg>
    </div>
  </div>
</section>

<section class="feature" id="details">
  <div class="wrap">
    <p class="kicker centered">And it's a real pet</p>
    <h2 class="h2 centered">PawBae is full of delightful details</h2>
    <div class="grid4">
      <div class="g4"><span class="gic">📺</span><h3>OBS stream stage</h3><p>A chroma-key mirror window drops your pet straight into your coding stream.</p></div>
      <div class="g4"><span class="gic">📔</span><h3>Pet diary</h3><p>It remembers your late-night fixes and greets you in the morning.</p></div>
      <div class="g4"><span class="gic">🥚</span><h3>Eggs &amp; dex</h3><p>Hatch cloud eggs, collect species, fill a gallery of silhouettes.</p></div>
      <div class="g4"><span class="gic">🎨</span><h3>Skin workshop</h3><p>Make your own skins and share them — your pet, your look.</p></div>
    </div>
  </div>
</section>

<section class="faq-sec" id="faq">
  <div class="wrap faq-grid">
    <div>
      <h2>Frequently asked questions</h2>
      <p class="faq-aside">Can't find what you're looking for? Ask us anything on GitHub.</p>
    </div>
    <div class="faq">
      <details><summary>Which coding agents does PawBae support?</summary><p>Claude Code, Codex and Cursor today, via their local hook events. More connectors are on the roadmap — the pet works fine without any agent, too.</p></details>
      <details><summary>Is my code or prompt data sent anywhere?</summary><p>No. Agent events are processed on your machine. The only thing friends can ever see is a five-state mood projection — never task content.</p></details>
      <details><summary>What platforms are supported?</summary><p>macOS (Apple silicon) is the primary platform; a Windows beta installer is available. Mobile companion is planned after the beta.</p></details>
      <details><summary>How do invites work?</summary><p>Every beta member gets a few invite codes to bring friends — visits are more fun with people you know. No invite? Join the waitlist below.</p></details>
      <details><summary>Is it free?</summary><p>The desktop pet is free during the closed beta. We'll be transparent about pricing long before that changes.</p></details>
    </div>
  </div>
</section>

<section class="wait-band" id="waitlist">
  <div class="wrap wait-grid">
    <div>
      <h2 class="h2 on-dark">PawBae is in closed beta</h2>
      <p class="sub2">We invite a few new humans every week. Leave your email and your pet will fetch you an invite.</p>
      <Waitlist />
      <p class="invite-note">Have an invite code? Redeem it inside the app after installing.</p>
    </div>
    <div class="wait-mini">
      <div class="wm"><span class="gic dark-gic">✉️</span><h3>A few invites weekly</h3><p>First come, first served — friends of current members get priority.</p></div>
      <div class="wm"><span class="gic dark-gic">🚫</span><h3>No spam, ever</h3><p>One email when it's your turn. That's the whole newsletter.</p></div>
    </div>
  </div>
</section>

<style>
  /* ============ hero ============ */
  .hero {
    background: var(--hero-bg);
    padding: 56px 0 0;
    overflow: hidden;
    position: relative;
  }
  .hero-in {
    text-align: center;
    position: relative;
  }
  .ship-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: 1px solid rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.75);
    border-radius: 999px;
    padding: 6px 16px;
    font-size: 13px;
    font-weight: 600;
    text-decoration: none;
  }
  .ship-pill .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--mint);
  }
  .ship-pill:hover {
    color: #fff;
  }
  .hero h1 {
    margin: 26px auto 0;
    font-size: clamp(38px, 7vw, 72px);
    font-weight: 700;
    letter-spacing: -0.03em;
    line-height: 1.05;
    color: rgba(255, 255, 255, 0.9);
    max-width: 13em;
    text-wrap: balance;
  }
  .hero .sub {
    margin: 22px auto 0;
    font-size: 18px;
    color: #9ca3af;
    max-width: 34em;
  }
  .ctas {
    margin-top: 32px;
    display: flex;
    gap: 14px;
    justify-content: center;
    flex-wrap: wrap;
  }
  .cta-note {
    margin-top: 16px;
    font-size: 13.5px;
    color: #6b7280;
  }
  .cta-note a {
    color: #9ca3af;
  }

  .hero-shot {
    position: relative;
    max-width: 880px;
    margin: 56px auto -2px;
  }
  .hero-glow {
    position: absolute;
    inset: -40px -80px 0;
    background: radial-gradient(closest-side, rgba(125, 211, 252, 0.16), transparent 70%);
    pointer-events: none;
  }
  .desk {
    position: relative;
    background: linear-gradient(180deg, #1b2437, #131a2a);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-bottom: none;
    border-radius: 18px 18px 0 0;
    padding: 42px 42px 0;
  }
  .term {
    background: #0d1117;
    border-radius: 10px 10px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-bottom: none;
    overflow: hidden;
    text-align: left;
  }
  .term-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 10px 12px;
    background: #161b22;
  }
  .term-bar .tl {
    width: 11px;
    height: 11px;
    border-radius: 50%;
  }
  .term-title {
    margin-left: 10px;
    font-size: 11.5px;
    color: #7d8590;
    font-family: var(--font-mono);
  }
  .term-body {
    padding: 14px 18px 22px;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.8;
    color: #a9b1d6;
  }
  .term-body .pr {
    color: #7dd3fc;
  }
  .term-body .ok {
    color: #34d399;
  }
  .term-body .dim {
    color: #565f89;
  }
  .pet-perch {
    position: absolute;
    top: -8px;
    right: 120px;
    width: 104px;
  }
  .pet-tail {
    transform-origin: 88px 66px;
    animation: wag 3.2s ease-in-out infinite;
  }
  @keyframes wag {
    0%,
    100% {
      transform: rotate(0);
    }
    50% {
      transform: rotate(9deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pet-tail {
      animation: none;
    }
  }
  .status-chip {
    position: absolute;
    top: 2px;
    right: 34px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.92);
    backdrop-filter: blur(8px);
    border-radius: 999px;
    padding: 4px 11px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-900);
    box-shadow: var(--card-shadow);
  }
  .status-chip .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--mint);
  }
  .hero-toast {
    position: absolute;
    top: -2px;
    left: 46px;
    background: rgba(255, 255, 255, 0.92);
    backdrop-filter: blur(8px);
    border-radius: 12px;
    padding: 9px 15px;
    font-size: 13px;
    font-weight: 600;
    color: var(--ink-900);
    box-shadow: var(--card-shadow);
  }

  /* ============ gray-950 工具带 ============ */
  .tools-band {
    background: var(--band-950);
    padding: 80px 0 92px;
    text-align: center;
  }
  .tools-band .sub2 {
    color: #9ca3af;
    font-size: 15px;
    margin-top: 14px;
  }
  .tool-row {
    display: flex;
    justify-content: center;
    gap: 14px;
    flex-wrap: wrap;
    margin-top: 36px;
  }
  .tool-mark {
    font-family: var(--font-mono);
    font-size: 15px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.65);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    padding: 10px 26px;
  }

  /* ============ 交替功能段 ============ */
  section.feature {
    padding: 96px 0;
  }
  .split {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 64px;
    align-items: center;
  }
  @media (max-width: 860px) {
    .split {
      grid-template-columns: 1fr;
      gap: 40px;
    }
  }
  .on-dark-sec {
    background: var(--band-900);
  }
  .on-dark-sec .lede {
    color: var(--dark-body);
  }
  .sky-kicker {
    color: #7dd3fc;
  }
  .dark-ic {
    background: rgba(255, 255, 255, 0.08);
  }
  .privacy-art {
    text-align: center;
  }
  .centered {
    text-align: center;
  }
  .h2.centered {
    margin-inline: auto;
  }

  .state-card {
    background: #f9fafb;
    border: 1px solid #e5e7eb;
    border-radius: 16px;
    padding: 26px;
    display: grid;
    gap: 10px;
  }
  .state-row {
    display: flex;
    align-items: center;
    gap: 12px;
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 12px;
    padding: 10px 16px;
    font-size: 14px;
    color: var(--ink-600);
    box-shadow: var(--card-shadow);
  }
  .state-row b {
    color: var(--ink-900);
    font-weight: 600;
    min-width: 7em;
    font-family: var(--font-mono);
    font-size: 13px;
  }
  .state-pet {
    text-align: center;
    padding-top: 8px;
  }

  /* ============ 串门（天空渐变段） ============ */
  .visit-band {
    background: var(--sky-band);
    padding: 96px 0;
  }
  .visit-band .split {
    align-items: center;
  }
  .friend-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 13px 16px;
  }
  .friend-row + .friend-row {
    border-top: 1px solid #f1f5f9;
  }
  .avatar {
    width: 38px;
    height: 38px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 19px;
    flex: none;
  }
  .friend-meta b {
    display: block;
    font-size: 14.5px;
    color: var(--ink-900);
  }
  .friend-meta span {
    font-size: 12.5px;
    color: var(--ink-400);
  }
  .friend-state {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-600);
  }
  .memory-card {
    overflow: hidden;
    max-width: 320px;
    margin: 26px 0 0 auto;
    transform: rotate(1.5deg);
  }
  @media (max-width: 860px) {
    .memory-card {
      margin: 26px auto 0;
    }
  }
  .memory-art {
    height: 150px;
    background: linear-gradient(160deg, #bae6fd, #ddd6fe 55%, #fed7aa);
  }
  .memory-body {
    padding: 14px 18px 16px;
  }
  .memory-body h3 {
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.02em;
    color: var(--ink-900);
    margin: 0;
  }
  .memory-body time {
    font-size: 12px;
    color: var(--ink-400);
  }

  /* ============ 细节四格 ============ */
  .grid4 {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 40px 32px;
    margin-top: 52px;
  }
  @media (max-width: 900px) {
    .grid4 {
      grid-template-columns: 1fr 1fr;
    }
  }
  .gic {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    font-size: 19px;
    background: #eff6ff;
    margin-bottom: 14px;
  }
  .g4 h3 {
    font-size: 15.5px;
    font-weight: 600;
    color: var(--ink-900);
    margin: 0;
  }
  .g4 p {
    font-size: 14px;
    margin: 6px 0 0;
  }

  /* ============ FAQ ============ */
  .faq-sec {
    padding: 88px 0;
    background: #f9fafb;
  }
  .faq-grid {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 56px;
  }
  @media (max-width: 800px) {
    .faq-grid {
      grid-template-columns: 1fr;
    }
  }
  .faq-grid h2 {
    font-size: 24px;
    font-weight: 700;
    line-height: 1.4;
    letter-spacing: -0.02em;
    color: var(--ink-900);
    margin: 0;
  }
  .faq-aside {
    font-size: 14.5px;
    margin-top: 14px;
  }
  .faq details {
    border-bottom: 1px solid #e5e7eb;
  }
  .faq details:first-child {
    border-top: 1px solid #e5e7eb;
  }
  .faq summary {
    cursor: pointer;
    font-weight: 600;
    font-size: 15.5px;
    color: var(--ink-900);
    padding: 18px 4px;
    list-style: none;
    display: flex;
    justify-content: space-between;
  }
  .faq summary::after {
    content: '+';
    color: var(--ink-400);
    font-weight: 400;
  }
  .faq details[open] summary::after {
    content: '–';
  }
  .faq p {
    font-size: 14.5px;
    padding: 0 4px 18px;
    max-width: 60ch;
    margin: 0;
  }

  /* ============ 深色候补band ============ */
  .wait-band {
    background: var(--band-900);
    padding: 88px 0;
  }
  .wait-grid {
    display: grid;
    grid-template-columns: 1.2fr 1fr;
    gap: 56px;
    align-items: center;
  }
  @media (max-width: 800px) {
    .wait-grid {
      grid-template-columns: 1fr;
    }
  }
  .wait-band .sub2 {
    color: var(--dark-body);
    font-size: 15.5px;
    margin-top: 14px;
    max-width: 34em;
  }
  .invite-note {
    font-size: 13px;
    color: #6b7280;
    margin-top: 14px;
  }
  .wait-mini {
    display: grid;
    gap: 24px;
  }
  .wm {
    display: grid;
    gap: 6px;
  }
  .dark-gic {
    background: rgba(255, 255, 255, 0.08);
  }
  .wm h3 {
    margin: 10px 0 0;
    font-size: 15px;
    font-weight: 600;
    color: #fff;
  }
  .wm p {
    font-size: 14px;
    color: var(--dark-body);
    margin: 0;
  }
</style>
