<script lang="ts">
  import Waitlist from '$lib/Waitlist.svelte';

  function handleSummaryKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== ' ') return;

    const details = (event.currentTarget as HTMLElement).closest('details');
    if (!details) return;

    event.preventDefault();
    details.open = !details.open;
  }
</script>

<header class="hero" id="download">
  <div class="wrap hero-inner">
    <div class="hero-copy">
      <a class="ship-pill" href="https://github.com/PawBae/PawBae-app/blob/main/CHANGELOG.md">
        <span class="status-dot working" aria-hidden="true"></span>
        <span>Just shipped v0.2.0</span>
        <span class="pill-link">Read the changelog</span>
      </a>

      <h1>Your agent. Your pet. Your desktop.</h1>
      <p class="hero-lede">
        PawBae turns Claude Code, Codex, and Cursor activity into a companion you can read at a
        glance.
      </p>

      <div class="hero-actions">
        <a class="btn btn-yellow" href="https://github.com/PawBae/PawBae-app/releases/latest">
          Download Now For Free
        </a>
        <a class="btn btn-ghost-dark" href="#features">See how it works</a>
      </div>
    </div>

    <div class="hero-stage">
      <div class="stage-aura" aria-hidden="true"></div>
      <div
        class="terminal-window"
        role="img"
        aria-label="Claude Code terminal finishing a flaky authentication test"
      >
        <div class="terminal-bar">
          <span class="window-dot close" aria-hidden="true"></span>
          <span class="window-dot minimize" aria-hidden="true"></span>
          <span class="window-dot maximize" aria-hidden="true"></span>
          <span class="terminal-title">claude - ~/projects/pawbae</span>
          <span class="terminal-branch">main</span>
        </div>
        <div class="terminal-body" aria-hidden="true">
          <p><span class="prompt">❯</span> claude "trace the flaky auth test"</p>
          <p><span class="tool">●</span> Read(src/routes/auth.test.ts)</p>
          <p class="terminal-line secondary">
            <span class="tool">●</span> Edit(src/lib/session.ts)
          </p>
          <p class="terminal-line secondary">
            <span class="tool">●</span> Bash(pnpm test auth)
          </p>
          <p><span class="success">✓</span> 18 tests passed</p>
          <p class="done">Done. The race condition is fixed.</p>
          <p class="cursor-line"><span class="prompt">❯</span><span class="cursor"></span></p>
        </div>
      </div>

      <svg
        class="bobo-perch"
        viewBox="0 0 320 205"
        role="img"
        aria-label="Bobo the PawBae pet resting on the terminal window"
      >
        <path
          class="bobo-tail"
          d="M248 142c49 7 67-24 43-54"
          fill="none"
          stroke="#fb923c"
          stroke-width="24"
          stroke-linecap="round"
        />
        <path
          d="M90 65 74 20l49 31M230 65l16-45-49 31"
          fill="#7dd3fc"
          stroke="#efc28f"
          stroke-width="7"
          stroke-linejoin="round"
        />
        <path d="m90 47-8-19 21 13m127 6 8-19-21 13" fill="#c4b5fd" />
        <ellipse cx="160" cy="96" rx="83" ry="67" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
        <path d="M118 91q12 14 25 0m34 0q12 14 25 0" fill="none" stroke="#734b2e" stroke-width="7" stroke-linecap="round" />
        <path d="m154 114 6 5 6-5m-6 5v8" fill="none" stroke="#734b2e" stroke-width="5" stroke-linecap="round" />
        <ellipse cx="106" cy="116" rx="13" ry="7" fill="#f9a889" opacity=".65" />
        <ellipse cx="214" cy="116" rx="13" ry="7" fill="#f9a889" opacity=".65" />
        <path d="M137 44q23-15 46 0" fill="none" stroke="#a78bfa" stroke-width="8" stroke-linecap="round" />
        <rect x="106" y="145" width="42" height="50" rx="20" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
        <rect x="172" y="145" width="42" height="50" rx="20" fill="#ffe3c2" stroke="#efc28f" stroke-width="7" />
        <path d="M117 181h20m46 0h20" stroke="#f7b38a" stroke-width="5" stroke-linecap="round" />
      </svg>

      <span class="agent-status">
        <span class="status-dot working" aria-hidden="true"></span>
        Claude Code is working
      </span>
    </div>
  </div>
</header>

<section class="tools-band" aria-labelledby="tools-title">
  <div class="wrap tools-inner">
    <h2 id="tools-title">Works with the tools you live in</h2>
    <p>Local hook events in. Clear presence out. Your code stays on your machine.</p>
    <div class="tool-row" aria-label="Supported coding agents">
      <span class="tool-mark"><span class="tool-glyph claude" aria-hidden="true"></span>Claude Code</span>
      <span class="tool-mark"><span class="tool-glyph codex" aria-hidden="true"></span>Codex</span>
      <span class="tool-mark"><span class="tool-glyph cursor-glyph" aria-hidden="true"></span>Cursor</span>
    </div>
  </div>
</section>

<section class="feature-section" id="features">
  <div class="wrap split">
    <div class="section-copy">
      <p class="kicker">Presence, not dashboards</p>
      <h2 class="h2">Know what your agent is doing</h2>
      <p class="lede">No status window to check. Your pet is the status, readable from across the room.</p>

      <div class="dl">
        <div class="row">
          <span class="ic ic-mint" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M5 9h14v9H5zM8 6h8v3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
              <path d="m9 13 2 2 4-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <b>Working.</b> Bobo pitches a tiny tent while long agent tasks run.
        </div>
        <div class="row">
          <span class="ic ic-peach" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M6 4h12v16H6zM9 9h6m-6 4h4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <b>Waiting on you.</b> A note appears when your agent needs approval.
        </div>
        <div class="row">
          <span class="ic ic-sky" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="m12 3 1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
              <path d="m18.5 16 .8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z" fill="currentColor" />
            </svg>
          </span>
          <b>Done. Really done.</b> One soft chirp. One tiny celebration. Sub-agents stay quiet.
        </div>
      </div>
    </div>

    <div class="state-stage" aria-label="PawBae agent states">
      <div class="state-stage-head">
        <div>
          <span class="stage-label">Agent presence</span>
          <strong>Bobo is listening</strong>
        </div>
        <span class="live-state"><span class="sdot mint"></span>live</span>
      </div>

      <div class="state-list">
        <div class="state-row active">
          <span class="sdot mint"></span>
          <b>working</b>
          <span>tent up, focus on</span>
        </div>
        <div class="state-row">
          <span class="sdot peach"></span>
          <b>waiting</b>
          <span>note in paw</span>
        </div>
        <div class="state-row">
          <span class="sdot lavender"></span>
          <b>compacting</b>
          <span>tidying clouds</span>
        </div>
        <div class="state-row">
          <span class="sdot offline"></span>
          <b>offline</b>
          <span>curled up</span>
        </div>
      </div>

      <div class="state-scene">
        <svg viewBox="0 0 390 210" role="img" aria-label="Bobo working beside a tiny blue tent">
          <path d="m38 184 88-118 88 118z" fill="#bae6fd" />
          <path d="m126 66 2 118m-90 0h176" fill="none" stroke="#0f5597" stroke-width="7" stroke-linejoin="round" />
          <path d="m126 112 45 72h-90z" fill="#7dd3fc" opacity=".85" />
          <ellipse cx="286" cy="154" rx="64" ry="48" fill="#ffe3c2" stroke="#efc28f" stroke-width="6" />
          <path d="m242 137-12-38 42 27m58 11 12-38-42 27" fill="#7dd3fc" stroke="#efc28f" stroke-width="6" stroke-linejoin="round" />
          <circle cx="271" cy="150" r="5" fill="#734b2e" />
          <circle cx="301" cy="150" r="5" fill="#734b2e" />
          <path d="m280 166 6 5 6-5" fill="none" stroke="#734b2e" stroke-width="4" stroke-linecap="round" />
          <path d="M344 168c35 0 37-25 22-34" fill="none" stroke="#fb923c" stroke-width="15" stroke-linecap="round" />
          <rect x="234" y="186" width="104" height="10" rx="5" fill="#dbeafe" />
        </svg>
      </div>
    </div>
  </div>
</section>

<section class="visit-band" id="visits">
  <div class="wrap split visit-split">
    <div class="section-copy">
      <h2 class="h2">Friends' pets come to visit</h2>
      <p class="lede">
        Send your pet to a friend's desktop for a short visit. Two pets, two agents, one shared
        memory.
      </p>

      <div class="dl">
        <div class="row">
          <span class="ic ic-sky" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M7 8h10l2 11H5zm3 0V6a2 2 0 0 1 4 0v2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <b>Send your pet over.</b> Pick a friend. Bobo packs a tiny bag.
        </div>
        <div class="row">
          <span class="ic ic-peach" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <circle cx="8" cy="10" r="3" fill="none" stroke="currentColor" stroke-width="1.8" />
              <circle cx="16" cy="10" r="3" fill="none" stroke="currentColor" stroke-width="1.8" />
              <path d="M5 19c.5-3 2-5 4-5m10 5c-.5-3-2-5-4-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <b>Two pets. One desk.</b> Each still mirrors its own human's agent.
        </div>
        <div class="row">
          <span class="ic ic-lavender" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <rect x="4" y="6" width="16" height="13" rx="2" fill="none" stroke="currentColor" stroke-width="1.8" />
              <circle cx="9" cy="11" r="2" fill="none" stroke="currentColor" stroke-width="1.8" />
              <path d="m6 17 4-3 3 2 3-4 2 2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </span>
          <b>Come home with a memory.</b> No points. No streaks. Just the moment.
        </div>
      </div>
    </div>

    <div class="visit-canvas">
      <div class="friends-panel glass-card">
        <div class="panel-title">
          <span>Friends nearby</span>
          <span class="friend-count">3 pets</span>
        </div>

        <div class="friend-row">
          <span class="avatar avatar-sky">
            <svg class="avatar-face" viewBox="0 0 44 44" aria-hidden="true">
              <path d="m10 19-2-9 10 6m16 3 2-9-10 6" fill="#7dd3fc" />
              <ellipse cx="22" cy="25" rx="14" ry="12" fill="#ffe3c2" />
              <circle cx="17" cy="24" r="1.7" fill="#734b2e" /><circle cx="27" cy="24" r="1.7" fill="#734b2e" />
              <path d="m20 29 2 1.5 2-1.5" fill="none" stroke="#734b2e" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </span>
          <span class="friend-meta"><b>Momo</b><span>@keyu · friends since May</span></span>
          <span class="friend-state"><span class="sdot mint"></span>working</span>
        </div>

        <div class="friend-row">
          <span class="avatar avatar-peach">
            <svg class="avatar-face" viewBox="0 0 44 44" aria-hidden="true">
              <path d="m10 19-2-9 10 6m16 3 2-9-10 6" fill="#fb923c" />
              <ellipse cx="22" cy="25" rx="14" ry="12" fill="#ffedd5" />
              <path d="M16 24q2 3 4 0m4 0q2 3 4 0" fill="none" stroke="#7c3f1d" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <span class="friend-meta"><b>Tofu</b><span>@sarahk · friends since June</span></span>
          <span class="friend-state"><span class="sdot peach"></span>idle</span>
        </div>

        <div class="friend-row">
          <span class="avatar avatar-slate">
            <svg class="avatar-face" viewBox="0 0 44 44" aria-hidden="true">
              <path d="m10 19-2-9 10 6m16 3 2-9-10 6" fill="#c4b5fd" />
              <ellipse cx="22" cy="25" rx="14" ry="12" fill="#f1f5f9" />
              <path d="M16 25h4m4 0h4" stroke="#64748b" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <span class="friend-meta"><b>Mochi</b><span>@devon · friends since June</span></span>
          <span class="friend-state"><span class="sdot offline"></span>offline</span>
        </div>
      </div>

      <article class="memory-card glass-card">
        <div class="memory-art">
          <svg viewBox="0 0 380 220" role="img" aria-label="Bobo and Momo celebrating a finished task together">
            <circle cx="310" cy="44" r="22" fill="#fff7cc" opacity=".9" />
            <path d="M0 165c56-22 95-9 141 4 69 20 119-30 239-4v55H0z" fill="#dbeafe" />
            <path d="M0 0h380v164c-60-20-117 13-183-4-80-20-133-11-197 5z" fill="url(#memorySky)" />
            <defs>
              <linearGradient id="memorySky" x1="0" y1="0" x2="380" y2="180" gradientUnits="userSpaceOnUse">
                <stop stop-color="#bae6fd" /><stop offset=".55" stop-color="#ddd6fe" /><stop offset="1" stop-color="#fed7aa" />
              </linearGradient>
            </defs>
            <g transform="translate(67 70)">
              <path d="m17 38-8-28 30 20m62 8 8-28-30 20" fill="#7dd3fc" />
              <ellipse cx="59" cy="65" rx="50" ry="43" fill="#ffe3c2" stroke="#efc28f" stroke-width="5" />
              <path d="M40 62q7 8 14 0m12 0q7 8 14 0" fill="none" stroke="#734b2e" stroke-width="4" stroke-linecap="round" />
              <path d="M103 77c26 2 31-15 20-27" fill="none" stroke="#fb923c" stroke-width="12" stroke-linecap="round" />
            </g>
            <g transform="translate(210 76)">
              <path d="m15 36-7-25 27 18m57 7 7-25-27 18" fill="#c4b5fd" />
              <ellipse cx="54" cy="61" rx="46" ry="39" fill="#eef2ff" stroke="#b9b2d9" stroke-width="5" />
              <path d="M36 59q7 8 14 0m10 0q7 8 14 0" fill="none" stroke="#4c4b68" stroke-width="4" stroke-linecap="round" />
            </g>
            <path d="m183 55 4 11 11 4-11 4-4 11-4-11-11-4 11-4z" fill="#fff" opacity=".9" />
          </svg>
        </div>
        <div class="memory-body">
          <span class="memory-label">Shared memory</span>
          <h3>Shipped it. Together.</h3>
          <time datetime="2026-07-08T19:28:00">Bobo × Momo · Tuesday evening · 28 min</time>
        </div>
      </article>
    </div>
  </div>
</section>

<section class="privacy-section" id="privacy">
  <div class="wrap split privacy-split">
    <div class="section-copy on-dark-sec">
      <p class="kicker privacy-kicker">Built for your machine</p>
      <h2 class="h2 on-dark">Local. Private. Yours.</h2>
      <p class="lede">Your pet reads the agent so you do not have to. Nobody else gets to.</p>

      <div class="dl">
        <div class="row">
          <span class="ic dark-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <rect x="4" y="5" width="16" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.8" />
              <path d="M9 20h6m-3-4v4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <b>On-device by default.</b> Prompts, code, and file paths stay on your machine.
        </div>
        <div class="row">
          <span class="ic dark-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M4 12c2.5-4 5-6 8-6s5.5 2 8 6c-2.5 4-5 6-8 6s-5.5-2-8-6Z" fill="none" stroke="currentColor" stroke-width="1.8" />
              <circle cx="12" cy="12" r="2.5" fill="none" stroke="currentColor" stroke-width="1.8" />
              <path d="m5 20 14-16" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <b>A mood, not your code.</b> Friends see five states. Never task text or repo names.
        </div>
        <div class="row">
          <span class="ic dark-ic" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <rect x="5" y="10" width="14" height="10" rx="2" fill="none" stroke="currentColor" stroke-width="1.8" />
              <path d="M8 10V8a4 4 0 0 1 8 0v2m-4 4v2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
          </span>
          <b>You hold the leash.</b> Decline, recall, mute, or block without penalties.
        </div>
      </div>
    </div>

    <div class="privacy-house">
      <svg viewBox="0 0 520 430" role="img" aria-label="Bobo guarding a private local home for agent activity">
        <defs>
          <linearGradient id="windowGlow" x1="0" y1="0" x2="1" y2="1">
            <stop stop-color="#7dd3fc" stop-opacity=".55" /><stop offset="1" stop-color="#a78bfa" stop-opacity=".18" />
          </linearGradient>
        </defs>
        <circle cx="260" cy="220" r="190" fill="#fff" opacity=".025" />
        <path d="m92 205 168-123 168 123" fill="none" stroke="#64748b" stroke-width="8" stroke-linecap="round" stroke-linejoin="round" />
        <path d="M124 185v178h272V185" fill="#0b1220" stroke="#64748b" stroke-width="8" stroke-linejoin="round" />
        <rect x="178" y="196" width="164" height="105" rx="14" fill="url(#windowGlow)" stroke="#94a3b8" stroke-width="5" />
        <path d="M200 224h120m-120 25h78m-78 25h98" stroke="#dbeafe" stroke-width="7" stroke-linecap="round" opacity=".72" />
        <path d="M358 260v-34a26 26 0 0 1 52 0v34" fill="none" stroke="#facc15" stroke-width="8" stroke-linecap="round" />
        <rect x="346" y="256" width="76" height="66" rx="14" fill="#facc15" />
        <circle cx="384" cy="282" r="7" fill="#29240a" />
        <path d="M384 289v12" stroke="#29240a" stroke-width="6" stroke-linecap="round" />
        <g transform="translate(94 263)">
          <path d="m25 43-9-30 32 21m67 9 9-30-32 21" fill="#7dd3fc" stroke="#efc28f" stroke-width="5" stroke-linejoin="round" />
          <ellipse cx="70" cy="70" rx="55" ry="47" fill="#ffe3c2" stroke="#efc28f" stroke-width="6" />
          <circle cx="53" cy="68" r="4" fill="#734b2e" /><circle cx="87" cy="68" r="4" fill="#734b2e" />
          <path d="m63 82 7 5 7-5" fill="none" stroke="#734b2e" stroke-width="4" stroke-linecap="round" />
          <path d="M119 83c32 1 35-20 20-31" fill="none" stroke="#fb923c" stroke-width="14" stroke-linecap="round" />
        </g>
      </svg>
    </div>
  </div>
</section>

<section class="details-section" id="details">
  <div class="wrap">
    <div class="details-heading">
      <h2 class="h2">Small details. Real attachment.</h2>
      <p class="lede">PawBae keeps the useful parts close and gives the quiet moments somewhere to live.</p>
    </div>

    <div class="details-grid">
      <article class="detail-card obs-card">
        <div class="detail-copy">
          <span class="detail-label">For streams</span>
          <h3>OBS stream stage</h3>
          <p>A clean chroma-key mirror drops Bobo into your coding stream.</p>
        </div>
        <div class="stage-preview" aria-hidden="true">
          <div class="stage-window-bar"><span></span><span></span><span></span></div>
          <div class="chroma-stage">
            <svg viewBox="0 0 210 150">
              <path d="m54 62-12-38 42 27m72 11 12-38-42 27" fill="#7dd3fc" stroke="#efc28f" stroke-width="6" stroke-linejoin="round" />
              <ellipse cx="105" cy="89" rx="63" ry="52" fill="#ffe3c2" stroke="#efc28f" stroke-width="6" />
              <circle cx="85" cy="87" r="5" fill="#734b2e" /><circle cx="125" cy="87" r="5" fill="#734b2e" />
              <path d="m98 103 7 5 7-5" fill="none" stroke="#734b2e" stroke-width="4" stroke-linecap="round" />
              <path d="M162 104c34 0 39-24 20-37" fill="none" stroke="#fb923c" stroke-width="15" stroke-linecap="round" />
            </svg>
          </div>
        </div>
      </article>

      <article class="detail-card diary-card">
        <div class="detail-copy">
          <span class="detail-label">For remembering</span>
          <h3>Pet diary</h3>
          <p>Late-night fixes, morning hellos, and the tiny moments between.</p>
        </div>
        <div class="diary-preview" aria-hidden="true">
          <div class="diary-date"><span>Tuesday</span><b>July 8</b></div>
          <div class="diary-line"><span class="mini-face blue"></span><span>We fixed the flaky test.</span></div>
          <div class="diary-line"><span class="mini-face peach"></span><span>Momo came by after dinner.</span></div>
        </div>
      </article>

      <article class="detail-card egg-card">
        <div class="egg-visual" aria-hidden="true">
          <span class="egg egg-one"></span>
          <span class="egg egg-two"></span>
          <span class="egg egg-three"></span>
          <span class="egg-spark spark-one"></span>
          <span class="egg-spark spark-two"></span>
        </div>
        <div class="detail-copy">
          <span class="detail-label">For discovering</span>
          <h3>Eggs &amp; dex</h3>
          <p>Hatch cloud eggs and fill a gallery of new silhouettes.</p>
        </div>
      </article>

      <article class="detail-card skin-card">
        <div class="skin-visual" aria-hidden="true">
          <div class="skin-pet">
            <span class="skin-ear left"></span><span class="skin-ear right"></span>
            <span class="skin-eye left"></span><span class="skin-eye right"></span>
          </div>
          <div class="swatches">
            <span class="swatch sky"></span><span class="swatch peach"></span><span class="swatch lavender"></span><span class="swatch mint"></span>
          </div>
        </div>
        <div class="detail-copy">
          <span class="detail-label">For making it yours</span>
          <h3>Skin workshop</h3>
          <p>Shape a look that belongs to your desktop and nobody else's.</p>
        </div>
      </article>
    </div>
  </div>
</section>

<section class="faq-section" id="faq">
  <div class="wrap faq-grid">
    <div class="faq-intro">
      <h2>Frequently asked questions</h2>
      <p>Still curious? Ask us anything on <a href="https://github.com/PawBae">GitHub</a>.</p>
    </div>

    <div class="faq">
      <details>
        <summary onkeydown={handleSummaryKeydown}>Which coding agents does PawBae support?</summary>
        <p>Claude Code, Codex, and Cursor today through their local hook events. The pet also works without an agent.</p>
      </details>
      <details>
        <summary onkeydown={handleSummaryKeydown}>Is my code or prompt data sent anywhere?</summary>
        <p>No. Agent events are processed on your machine. Friends can only see a five-state mood projection, never task content.</p>
      </details>
      <details>
        <summary onkeydown={handleSummaryKeydown}>What platforms are supported?</summary>
        <p>macOS on Apple silicon is the primary platform. A Windows beta installer is also available.</p>
      </details>
      <details>
        <summary onkeydown={handleSummaryKeydown}>How do invites work?</summary>
        <p>Beta members receive a few invite codes for friends. If you do not have one, join the waitlist below.</p>
      </details>
      <details>
        <summary onkeydown={handleSummaryKeydown}>Is it free?</summary>
        <p>The desktop pet is free during the closed beta. We will share pricing clearly before that changes.</p>
      </details>
    </div>
  </div>
</section>

<section class="wait-band" id="waitlist">
  <div class="wrap wait-grid">
    <div class="wait-copy">
      <h2 class="h2 on-dark">Meet your desktop companion</h2>
      <p class="wait-lede">Download PawBae for free, or leave your email for the next closed beta invite.</p>
      <Waitlist />
      <p class="invite-note">Already have an invite? Redeem it inside the app after installing.</p>
    </div>

    <div class="wait-promises">
      <div class="promise">
        <span class="promise-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24"><path d="M4 7h16v11H4zm0 1 8 6 8-6" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" /></svg>
        </span>
        <div><h3>A few invites every week</h3><p>Current members can bring their friends first.</p></div>
      </div>
      <div class="promise">
        <span class="promise-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24"><path d="M12 3 5 6v5c0 4.6 2.8 8.1 7 10 4.2-1.9 7-5.4 7-10V6z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" /><path d="m9 12 2 2 4-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </span>
        <div><h3>No spam. Ever.</h3><p>One email when it is your turn. That is the whole newsletter.</p></div>
      </div>
    </div>
  </div>
</section>

<style>
  .hero {
    position: relative;
    isolation: isolate;
    overflow: hidden;
    background:
      radial-gradient(circle at 78% 68%, rgba(125, 211, 252, 0.08), transparent 31%),
      radial-gradient(circle at 62% 72%, rgba(167, 139, 250, 0.06), transparent 28%),
      var(--hero-bg);
    padding: clamp(4.75rem, 8vw, 7rem) 0 0;
  }

  .hero-copy {
    position: relative;
    z-index: 2;
    max-width: 49rem;
  }

  .ship-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.65rem;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 999px;
    padding: 0.45rem 0.8rem;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.72);
    font-size: 0.8rem;
    font-weight: 650;
    line-height: 1.45;
    text-decoration: none;
    transition:
      border-color 180ms ease,
      background-color 180ms ease,
      color 180ms ease;
  }

  .ship-pill:hover {
    border-color: rgba(255, 255, 255, 0.28);
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
  }

  .pill-link {
    border-left: 1px solid rgba(255, 255, 255, 0.18);
    padding-left: 0.65rem;
    color: rgba(255, 255, 255, 0.9);
  }

  .hero h1 {
    max-width: 12ch;
    margin: 1.6rem 0 0;
    color: rgba(255, 255, 255, 0.92);
    font-size: clamp(2.5rem, 6.4vw, 4.5rem);
    font-weight: 760;
    letter-spacing: -0.035em;
    line-height: 1.02;
    text-wrap: balance;
  }

  .hero-lede {
    max-width: 39rem;
    margin: 1.45rem 0 0;
    color: #a8b0bd;
    font-size: clamp(1.05rem, 1.8vw, 1.2rem);
    line-height: 1.65;
    text-wrap: pretty;
  }

  .hero-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.875rem;
    margin-top: 2rem;
  }

  .hero-stage {
    position: relative;
    z-index: 1;
    min-height: 34rem;
    margin-top: clamp(4.5rem, 8vw, 7rem);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-bottom: 0;
    border-radius: var(--radius-card) var(--radius-card) 0 0;
    padding: 5.25rem clamp(1rem, 4.5vw, 3.5rem) 0;
    background: linear-gradient(180deg, #202a40 0%, #121a2a 100%);
  }

  .stage-aura {
    position: absolute;
    inset: -8rem 15% auto;
    height: 18rem;
    border-radius: 999px;
    background: rgba(125, 211, 252, 0.1);
    filter: blur(80px);
    pointer-events: none;
  }

  .terminal-window {
    position: relative;
    z-index: 1;
    overflow: hidden;
    min-height: 27rem;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: var(--radius-card) var(--radius-card) 0 0;
    background: #080d16;
    box-shadow: 0 34px 80px -42px rgba(0, 0, 0, 0.88);
  }

  .terminal-bar {
    display: flex;
    min-height: 3.25rem;
    align-items: center;
    gap: 0.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    padding: 0 1rem;
    background: #151b27;
  }

  .window-dot {
    width: 0.72rem;
    height: 0.72rem;
    flex: none;
    border-radius: 999px;
  }

  .window-dot.close { background: #ff5f57; }
  .window-dot.minimize { background: #febc2e; }
  .window-dot.maximize { background: #28c840; }

  .terminal-title,
  .terminal-branch {
    color: #7c8594;
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }

  .terminal-title {
    margin-left: 0.55rem;
  }

  .terminal-branch {
    margin-left: auto;
    color: #596373;
  }

  .terminal-body {
    padding: clamp(1.75rem, 4vw, 3rem);
    color: #b9c1d8;
    font-family: var(--font-mono);
    font-size: clamp(0.78rem, 1.4vw, 0.95rem);
    line-height: 1.9;
  }

  .terminal-body p {
    overflow: hidden;
    margin: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .terminal-body p + p {
    margin-top: 0.35rem;
  }

  .prompt { color: #7dd3fc; }
  .tool { color: #c4b5fd; }
  .success { color: #34d399; }
  .done { color: #d7e2f2; }
  .cursor-line { color: #7dd3fc; }

  .cursor {
    display: inline-block;
    width: 0.55rem;
    height: 1em;
    margin-left: 0.5rem;
    vertical-align: -0.15em;
    background: #7dd3fc;
  }

  .bobo-perch {
    position: absolute;
    z-index: 3;
    top: -1.75rem;
    right: 5%;
    width: min(29vw, 20rem);
    min-width: 15rem;
    overflow: visible;
    filter: drop-shadow(0 18px 24px rgba(0, 0, 0, 0.22));
  }

  .bobo-tail {
    transform-origin: 255px 137px;
  }

  .agent-status {
    position: absolute;
    z-index: 4;
    right: clamp(1.5rem, 5vw, 4.5rem);
    bottom: 1.5rem;
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    border: 1px solid rgba(255, 255, 255, 0.72);
    border-radius: 999px;
    padding: 0.58rem 0.9rem;
    background: rgba(255, 255, 255, 0.92);
    color: var(--ink-900);
    font-size: 0.78rem;
    font-weight: 700;
    box-shadow: 0 12px 30px -20px rgba(2, 6, 23, 0.8);
    -webkit-backdrop-filter: blur(12px);
    backdrop-filter: blur(12px);
  }

  .tools-band {
    background: var(--band-950);
    padding: clamp(4.75rem, 8vw, 6rem) 0;
    text-align: center;
  }

  .tools-inner {
    display: grid;
    justify-items: center;
  }

  .tools-band h2 {
    margin: 0;
    color: rgba(255, 255, 255, 0.94);
    font-size: clamp(2rem, 4vw, 2.5rem);
    font-weight: 740;
    letter-spacing: -0.035em;
    line-height: 1.1;
    text-wrap: balance;
  }

  .tools-band p {
    max-width: 42rem;
    margin: 1rem 0 0;
    color: #94a3b8;
    font-size: 0.98rem;
  }

  .tool-row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.75rem;
    margin-top: 2rem;
  }

  .tool-mark {
    display: inline-flex;
    min-height: 3rem;
    align-items: center;
    gap: 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.13);
    border-radius: var(--radius-button);
    padding: 0.7rem 1.2rem;
    background: rgba(255, 255, 255, 0.035);
    color: #d3dae6;
    font-size: 0.9rem;
    font-weight: 650;
  }

  .tool-glyph {
    position: relative;
    display: inline-block;
    width: 1rem;
    height: 1rem;
  }

  .tool-glyph.claude {
    border: 2px solid #fb923c;
    border-radius: 0.28rem;
    transform: rotate(45deg);
  }

  .tool-glyph.codex {
    border: 2px solid #7dd3fc;
    border-radius: 999px;
  }

  .tool-glyph.codex::after {
    position: absolute;
    inset: 0.2rem;
    border-radius: 999px;
    background: #7dd3fc;
    content: '';
  }

  .tool-glyph.cursor-glyph {
    border-top: 0.95rem solid #c4b5fd;
    border-right: 0.48rem solid transparent;
    border-left: 0.48rem solid transparent;
  }

  .feature-section,
  .details-section {
    padding: var(--section-space) 0;
    background: #fff;
  }

  .split {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    align-items: center;
    gap: clamp(3rem, 7vw, 6.5rem);
  }

  .section-copy {
    min-width: 0;
  }

  .ic-mint { background: #ecfdf5; color: #047857; }
  .ic-peach { background: #fff7ed; color: #c2410c; }
  .ic-sky { background: #eff6ff; color: #0562c0; }
  .ic-lavender { background: #f5f3ff; color: #7c3aed; }

  .state-stage {
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: var(--surface-soft);
    box-shadow: var(--card-shadow);
  }

  .state-stage-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    border-bottom: 1px solid var(--line);
    padding: 1.2rem 1.35rem;
    background: rgba(255, 255, 255, 0.9);
  }

  .state-stage-head > div {
    display: grid;
    gap: 0.15rem;
  }

  .stage-label {
    color: var(--ink-500);
    font-size: 0.72rem;
    font-weight: 650;
  }

  .state-stage-head strong {
    color: var(--ink-900);
    font-size: 0.98rem;
    letter-spacing: -0.02em;
  }

  .live-state {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    border-radius: 999px;
    padding: 0.35rem 0.65rem;
    background: #ecfdf5;
    color: #047857;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .state-list {
    display: grid;
    gap: 0.55rem;
    padding: 1.1rem 1.1rem 0;
  }

  .state-row {
    display: grid;
    grid-template-columns: auto minmax(6.5rem, auto) 1fr;
    align-items: center;
    gap: 0.75rem;
    border: 1px solid transparent;
    border-radius: var(--radius-button);
    padding: 0.65rem 0.8rem;
    color: var(--ink-500);
    font-size: 0.78rem;
  }

  .state-row.active {
    border-color: #d1fae5;
    background: #fff;
  }

  .state-row b {
    color: var(--ink-900);
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }

  .sdot.mint { background: var(--mint); }
  .sdot.peach { background: var(--peach); }
  .sdot.lavender { background: var(--lavender); }
  .sdot.offline { background: #94a3b8; }

  .state-scene {
    padding: 0.25rem 1.2rem 0.9rem;
  }

  .state-scene svg {
    width: 100%;
    max-height: 14rem;
  }

  .visit-band {
    padding: var(--section-space) 0;
    background: linear-gradient(150deg, #e0f2fe 0%, #ede9fe 55%, #fff7ed 100%);
  }

  .visit-split {
    align-items: center;
  }

  .visit-canvas {
    position: relative;
    min-width: 0;
    padding: 0.75rem 0 4rem;
  }

  .friends-panel {
    position: relative;
    z-index: 2;
    overflow: hidden;
  }

  .panel-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid #f1f5f9;
    padding: 1rem 1.1rem;
    color: var(--ink-900);
    font-size: 0.82rem;
    font-weight: 700;
  }

  .friend-count {
    color: var(--ink-500);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .friend-row {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.8rem;
    padding: 0.85rem 1.1rem;
  }

  .friend-row + .friend-row {
    border-top: 1px solid #f1f5f9;
  }

  .avatar {
    display: grid;
    width: 2.6rem;
    height: 2.6rem;
    flex: none;
    place-items: center;
    overflow: hidden;
    border-radius: 999px;
  }

  .avatar-sky { background: #e0f2fe; }
  .avatar-peach { background: #ffedd5; }
  .avatar-slate { background: #f1f5f9; }
  .avatar-face { width: 2.5rem; height: 2.5rem; }

  .friend-meta {
    display: grid;
    min-width: 0;
    flex: 1;
  }

  .friend-meta b {
    color: var(--ink-900);
    font-size: 0.88rem;
  }

  .friend-meta span {
    overflow: hidden;
    color: var(--ink-500);
    font-size: 0.72rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .friend-state {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 0.45rem;
    color: var(--ink-600);
    font-size: 0.7rem;
    font-weight: 650;
  }

  .memory-card {
    position: relative;
    z-index: 3;
    width: min(78%, 22rem);
    overflow: hidden;
    margin: -0.8rem 0 0 auto;
    transform: rotate(1.5deg);
    transition:
      transform 240ms cubic-bezier(0.16, 1, 0.3, 1),
      box-shadow 240ms ease;
  }

  .memory-card:hover {
    transform: rotate(0deg) translateY(-3px);
    box-shadow: 0 20px 44px -30px rgba(15, 23, 42, 0.42);
  }

  .memory-art {
    aspect-ratio: 19 / 11;
    overflow: hidden;
  }

  .memory-art svg {
    width: 100%;
    height: 100%;
  }

  .memory-body {
    padding: 1rem 1.1rem 1.1rem;
    background: rgba(255, 255, 255, 0.96);
  }

  .memory-label,
  .detail-label {
    color: var(--primary);
    font-size: 0.72rem;
    font-weight: 700;
  }

  .memory-body h3 {
    margin: 0.2rem 0 0;
    color: var(--ink-900);
    font-size: 1.15rem;
    font-weight: 750;
    letter-spacing: -0.025em;
  }

  .memory-body time {
    display: block;
    margin-top: 0.15rem;
    color: var(--ink-500);
    font-size: 0.68rem;
  }

  .privacy-section {
    padding: var(--section-space) 0;
    overflow: hidden;
    background:
      radial-gradient(circle at 80% 45%, rgba(125, 211, 252, 0.06), transparent 30%),
      var(--band-900);
  }

  .privacy-split {
    align-items: center;
  }

  .privacy-kicker {
    color: var(--sky);
  }

  .on-dark-sec .lede,
  .on-dark-sec .dl .row {
    color: #c3ccd9;
  }

  .on-dark-sec .dl .row b {
    color: #fff;
  }

  .dark-ic {
    background: rgba(255, 255, 255, 0.08);
    color: #bae6fd;
  }

  .privacy-house {
    min-width: 0;
  }

  .privacy-house svg {
    width: 100%;
    max-height: 31rem;
    overflow: visible;
    filter: drop-shadow(0 30px 50px rgba(0, 0, 0, 0.2));
  }

  .details-heading {
    display: grid;
    justify-items: center;
    text-align: center;
  }

  .details-heading .h2 {
    max-width: 15ch;
  }

  .details-heading .lede {
    max-width: 38rem;
  }

  .details-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1rem;
    margin-top: clamp(3rem, 6vw, 4.5rem);
  }

  .detail-card {
    position: relative;
    display: flex;
    min-width: 0;
    min-height: 27rem;
    flex-direction: column;
    justify-content: space-between;
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: var(--radius-card);
    background: #fff;
    box-shadow: var(--card-shadow);
  }

  .detail-copy {
    position: relative;
    z-index: 2;
    padding: 1.6rem;
  }

  .detail-copy h3 {
    margin: 0.35rem 0 0;
    color: var(--ink-900);
    font-size: clamp(1.35rem, 3vw, 1.75rem);
    font-weight: 750;
    letter-spacing: -0.035em;
    line-height: 1.15;
  }

  .detail-copy p {
    max-width: 30rem;
    margin: 0.6rem 0 0;
    color: var(--ink-600);
    font-size: 0.92rem;
    line-height: 1.6;
  }

  .obs-card {
    grid-column: span 7;
    background: var(--band-950);
  }

  .obs-card .detail-label { color: #7dd3fc; }
  .obs-card .detail-copy h3 { color: #fff; }
  .obs-card .detail-copy p { color: #a8b0bd; }

  .stage-preview {
    width: 78%;
    margin: 0 auto -1.25rem;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: var(--radius-card) var(--radius-card) 0 0;
    background: #0b1220;
    box-shadow: 0 24px 50px -28px rgba(0, 0, 0, 0.85);
  }

  .stage-window-bar {
    display: flex;
    gap: 0.35rem;
    padding: 0.7rem;
    background: #151b27;
  }

  .stage-window-bar span {
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 999px;
    background: #475569;
  }

  .chroma-stage {
    display: grid;
    min-height: 13rem;
    place-items: end center;
    background: #00d968;
  }

  .chroma-stage svg {
    width: 72%;
  }

  .diary-card {
    grid-column: span 5;
    background: linear-gradient(150deg, #f5f3ff, #fff 55%, #eff6ff);
  }

  .diary-preview {
    display: grid;
    gap: 0.7rem;
    margin: 0 1.6rem 1.6rem;
    border: 1px solid rgba(255, 255, 255, 0.85);
    border-radius: var(--radius-card);
    padding: 1.1rem;
    background: rgba(255, 255, 255, 0.78);
    box-shadow: 0 12px 30px -24px rgba(67, 56, 202, 0.32);
  }

  .diary-date {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    color: var(--ink-500);
    font-size: 0.72rem;
  }

  .diary-date b {
    color: var(--ink-900);
    font-size: 0.82rem;
  }

  .diary-line {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    border-top: 1px solid #eef2f7;
    padding-top: 0.7rem;
    color: var(--ink-700);
    font-size: 0.76rem;
  }

  .mini-face {
    position: relative;
    width: 1.8rem;
    height: 1.5rem;
    flex: none;
    border-radius: 48% 48% 45% 45%;
    background: #ffe3c2;
  }

  .mini-face::before,
  .mini-face::after {
    position: absolute;
    top: -0.25rem;
    width: 0.65rem;
    height: 0.65rem;
    background: var(--ear-color);
    content: '';
    transform: rotate(45deg);
  }

  .mini-face::before { left: 0.15rem; }
  .mini-face::after { right: 0.15rem; }
  .mini-face.blue { --ear-color: #7dd3fc; }
  .mini-face.peach { --ear-color: #fb923c; }

  .egg-card {
    grid-column: span 6;
    background: linear-gradient(155deg, #ecfdf5, #f8fafc);
  }

  .egg-visual {
    position: relative;
    display: flex;
    min-height: 16rem;
    align-items: end;
    justify-content: center;
    gap: 1rem;
    padding: 2rem 1.5rem 0;
  }

  .egg {
    position: relative;
    display: block;
    width: 5.2rem;
    height: 6.6rem;
    border: 5px solid rgba(15, 118, 110, 0.16);
    border-radius: 52% 48% 46% 54% / 60% 60% 40% 40%;
    background: #fff;
    box-shadow: inset -0.8rem -0.8rem 0 rgba(125, 211, 252, 0.12);
  }

  .egg::after {
    position: absolute;
    top: 2.1rem;
    left: 50%;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 999px;
    background: var(--egg-color);
    content: '';
    transform: translateX(-50%);
  }

  .egg-one { --egg-color: #7dd3fc; transform: rotate(-8deg) translateY(1rem); }
  .egg-two { --egg-color: #a78bfa; width: 6rem; height: 7.6rem; }
  .egg-three { --egg-color: #fb923c; transform: rotate(8deg) translateY(1rem); }

  .egg-spark {
    position: absolute;
    width: 0.65rem;
    height: 0.65rem;
    background: var(--yellow);
    clip-path: polygon(50% 0, 62% 38%, 100% 50%, 62% 62%, 50% 100%, 38% 62%, 0 50%, 38% 38%);
  }

  .spark-one { top: 2.4rem; left: 26%; }
  .spark-two { top: 4.2rem; right: 22%; }

  .skin-card {
    grid-column: span 6;
    background: #f8fafc;
  }

  .skin-visual {
    display: grid;
    min-height: 16rem;
    place-items: center;
    padding: 2rem 1.5rem 0;
  }

  .skin-pet {
    position: relative;
    width: 9rem;
    height: 7.6rem;
    border: 5px solid #efc28f;
    border-radius: 48% 48% 45% 45%;
    background: #ffe3c2;
    box-shadow: inset 0 1.2rem 0 #e0f2fe;
  }

  .skin-ear {
    position: absolute;
    top: -1.8rem;
    width: 3.2rem;
    height: 3.2rem;
    border: 5px solid #efc28f;
    background: #7dd3fc;
    transform: rotate(45deg);
  }

  .skin-ear.left { left: 0.6rem; }
  .skin-ear.right { right: 0.6rem; }

  .skin-eye {
    position: absolute;
    top: 4rem;
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 999px;
    background: #734b2e;
  }

  .skin-eye.left { left: 3rem; }
  .skin-eye.right { right: 3rem; }

  .swatches {
    display: flex;
    gap: 0.55rem;
    margin-top: 1.4rem;
  }

  .swatch {
    width: 1.25rem;
    height: 1.25rem;
    border: 2px solid #fff;
    border-radius: 999px;
    box-shadow: 0 0 0 1px #cbd5e1;
  }

  .swatch.sky { background: var(--sky); }
  .swatch.peach { background: var(--peach); }
  .swatch.lavender { background: var(--lavender); }
  .swatch.mint { background: var(--mint); }

  .faq-section {
    padding: clamp(5rem, 8vw, 6.5rem) 0;
    background: var(--surface-soft);
  }

  .faq-grid {
    display: grid;
    grid-template-columns: minmax(14rem, 18rem) minmax(0, 1fr);
    align-items: start;
    gap: clamp(3rem, 7vw, 6rem);
  }

  .faq-intro {
    position: static;
  }

  .faq-intro h2 {
    margin: 0;
    color: var(--ink-900);
    font-size: 1.55rem;
    font-weight: 750;
    letter-spacing: -0.03em;
    line-height: 1.25;
  }

  .faq-intro p {
    margin: 0.8rem 0 0;
    color: var(--ink-600);
    font-size: 0.9rem;
  }

  .faq-intro a {
    color: var(--primary);
    font-weight: 700;
  }

  .faq details {
    border-bottom: 1px solid #dbe3ee;
  }

  .faq details:first-child {
    border-top: 1px solid #dbe3ee;
  }

  .faq summary {
    position: relative;
    display: block;
    min-height: 4.25rem;
    padding: 1.25rem 2.5rem 1.25rem 0;
    cursor: pointer;
    color: var(--ink-900);
    font-size: 0.98rem;
    font-weight: 700;
    line-height: 1.6;
    list-style: none;
  }

  .faq summary::-webkit-details-marker {
    display: none;
  }

  .faq summary::before,
  .faq summary::after {
    position: absolute;
    top: 50%;
    right: 0.25rem;
    width: 0.9rem;
    height: 2px;
    background: var(--ink-500);
    content: '';
    transition:
      transform 180ms ease,
      opacity 180ms ease;
  }

  .faq summary::before {
    transform: translateY(-50%) rotate(90deg);
  }

  .faq summary::after {
    transform: translateY(-50%);
  }

  .faq details[open] summary::before {
    opacity: 0;
    transform: translateY(-50%) rotate(0);
  }

  .faq details p {
    max-width: 60ch;
    margin: -0.25rem 0 0;
    padding: 0 2.5rem 1.4rem 0;
    color: var(--ink-600);
    font-size: 0.91rem;
    line-height: 1.75;
  }

  .wait-band {
    padding: clamp(5rem, 9vw, 7rem) 0;
    background:
      radial-gradient(circle at 78% 30%, rgba(125, 211, 252, 0.08), transparent 27%),
      var(--band-900);
  }

  .wait-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.12fr) minmax(18rem, 0.88fr);
    align-items: center;
    gap: clamp(3rem, 8vw, 7rem);
  }

  .wait-lede {
    max-width: 36rem;
    margin: 1rem 0 0;
    color: #c3ccd9;
    font-size: 1rem;
  }

  .invite-note {
    margin: 0.85rem 0 0;
    color: #94a3b8;
    font-size: 0.78rem;
  }

  .wait-promises {
    display: grid;
    gap: 1.5rem;
  }

  .promise {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: start;
    gap: 1rem;
  }

  .promise-icon {
    display: grid;
    width: 2.75rem;
    height: 2.75rem;
    place-items: center;
    border-radius: var(--radius-button);
    background: rgba(255, 255, 255, 0.08);
    color: #bae6fd;
  }

  .promise-icon svg {
    width: 1.25rem;
    height: 1.25rem;
  }

  .promise h3 {
    margin: 0;
    color: #fff;
    font-size: 0.98rem;
    font-weight: 700;
  }

  .promise p {
    margin: 0.25rem 0 0;
    color: #a8b0bd;
    font-size: 0.86rem;
  }

  @media (prefers-reduced-motion: no-preference) {
    .hero-copy > * {
      animation: hero-enter 720ms cubic-bezier(0.16, 1, 0.3, 1) both;
    }

    .hero-copy > :nth-child(2) { animation-delay: 70ms; }
    .hero-copy > :nth-child(3) { animation-delay: 130ms; }
    .hero-copy > :nth-child(4) { animation-delay: 190ms; }

    .hero-stage {
      animation: stage-enter 900ms 180ms cubic-bezier(0.16, 1, 0.3, 1) both;
    }

    .bobo-tail {
      animation: tail-wag 3.8s ease-in-out infinite;
    }

    .status-dot.working {
      animation: status-pulse 2.8s ease-out infinite;
    }

    .cursor {
      animation: cursor-blink 1.05s steps(2, jump-none) infinite;
    }
  }

  @keyframes hero-enter {
    from { transform: translateY(18px); }
    to { transform: translateY(0); }
  }

  @keyframes stage-enter {
    from { transform: translateY(28px) scale(0.985); }
    to { transform: translateY(0) scale(1); }
  }

  @keyframes tail-wag {
    0%, 100% { transform: rotate(0deg); }
    50% { transform: rotate(8deg); }
  }

  @keyframes status-pulse {
    0%, 55%, 100% { box-shadow: 0 0 0 4px rgba(52, 211, 153, 0.12); }
    75% { box-shadow: 0 0 0 7px rgba(52, 211, 153, 0); }
  }

  @keyframes cursor-blink {
    0%, 48% { opacity: 1; }
    50%, 100% { opacity: 0; }
  }

  @media (max-width: 900px) {
    .split,
    .faq-grid,
    .wait-grid {
      grid-template-columns: 1fr;
    }

    .state-stage,
    .visit-canvas,
    .privacy-house {
      width: min(100%, 38rem);
      margin-inline: auto;
    }

    .faq-intro p {
      max-width: 34rem;
    }

    .wait-promises {
      grid-template-columns: 1fr 1fr;
    }
  }

  @media (max-width: 720px) {
    .details-grid {
      grid-template-columns: 1fr;
    }

    .detail-card,
    .obs-card,
    .diary-card,
    .egg-card,
    .skin-card {
      grid-column: 1;
    }

    .detail-card {
      min-height: 24rem;
    }
  }

  @media (max-width: 640px) {
    .hero {
      padding-top: 4.5rem;
    }

    .hero h1 {
      max-width: 9ch;
      font-size: clamp(2.5rem, 12vw, 3.25rem);
    }

    .hero-lede {
      font-size: 1.05rem;
    }

    .hero-actions {
      align-items: stretch;
      flex-direction: column;
    }

    .hero-actions .btn {
      width: 100%;
      text-align: center;
    }

    .pill-link {
      display: none;
    }

    .hero-stage {
      min-height: 27rem;
      margin-top: 5.25rem;
      padding: 3.3rem 0.7rem 0;
    }

    .terminal-window {
      min-height: 23.5rem;
    }

    .terminal-bar {
      min-height: 2.8rem;
      gap: 0.38rem;
      padding-inline: 0.75rem;
    }

    .window-dot {
      width: 0.6rem;
      height: 0.6rem;
    }

    .terminal-title {
      max-width: 10rem;
      overflow: hidden;
      font-size: 0.61rem;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .terminal-branch {
      display: none;
    }

    .terminal-body {
      min-height: 20rem;
      padding: 1.6rem 1rem;
      font-size: 0.66rem;
      line-height: 1.85;
    }

    .terminal-line.secondary {
      display: none;
    }

    .bobo-perch {
      top: -1.6rem;
      right: -0.25rem;
      width: 12rem;
      min-width: 12rem;
    }

    .agent-status {
      right: 1rem;
      bottom: 1rem;
      padding: 0.5rem 0.7rem;
      font-size: 0.68rem;
    }

    .tool-row {
      width: 100%;
      flex-direction: column;
    }

    .tool-mark {
      justify-content: center;
    }

    .state-row {
      grid-template-columns: auto 6.7rem 1fr;
      gap: 0.55rem;
      font-size: 0.7rem;
    }

    .visit-canvas {
      padding-bottom: 0;
    }

    .friend-row {
      padding-inline: 0.9rem;
    }

    .friend-meta span {
      max-width: 8rem;
    }

    .memory-card {
      width: calc(100% - 1.5rem);
      margin-top: 1rem;
    }

    .privacy-house svg {
      max-height: 25rem;
    }

    .stage-preview {
      width: calc(100% - 2rem);
    }

    .egg {
      width: 4.3rem;
      height: 5.5rem;
    }

    .wait-promises {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 380px) {
    .hero h1 {
      font-size: 2.45rem;
    }

    .ship-pill {
      gap: 0.5rem;
      font-size: 0.74rem;
    }

    .friend-state {
      font-size: 0;
    }

    .friend-state .sdot {
      width: 0.6rem;
      height: 0.6rem;
    }

    .egg-visual {
      gap: 0.55rem;
    }

    .egg {
      width: 3.8rem;
      height: 4.8rem;
    }
  }
</style>
