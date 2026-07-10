<script lang="ts">
  import '../app.css';

  let { children } = $props();
</script>

<svelte:head>
  <title>PawBae — The desktop pet that watches your AI agents</title>
  <meta
    name="description"
    content="PawBae is a tiny desktop companion that lives beside your coding agent — it works when Claude Code works, waits when it waits, and celebrates when you ship."
  />
</svelte:head>

<nav>
  <div class="wrap nav-in">
    <a class="logo" href="/">
      <svg width="26" height="26" viewBox="0 0 26 26" aria-hidden="true">
        <ellipse cx="13" cy="16" rx="8.5" ry="6.5" fill="#7dd3fc" />
        <circle cx="6" cy="9" r="3" fill="#c4b5fd" /><circle cx="13" cy="7" r="3.2" fill="#7dd3fc" /><circle
          cx="20"
          cy="9"
          r="3"
          fill="#fdba74"
        />
      </svg>
      PawBae
    </a>
    <div class="nav-links">
      <a href="/#features">Features</a><a href="/#visits">Visits</a><a href="/#privacy">Privacy</a><a
        href="/#faq">FAQ</a
      >
    </div>
    <a class="btn btn-yellow nav-cta" href="/#download">Download</a>
  </div>
</nav>

{@render children()}

<footer>
  <div class="wrap">
    <div class="foot-grid">
      <div>
        <a class="logo foot-logo" href="/">
          <svg width="24" height="24" viewBox="0 0 26 26" aria-hidden="true">
            <ellipse cx="13" cy="16" rx="8.5" ry="6.5" fill="#7dd3fc" />
            <circle cx="6" cy="9" r="3" fill="#c4b5fd" /><circle cx="13" cy="7" r="3.2" fill="#7dd3fc" /><circle
              cx="20"
              cy="9"
              r="3"
              fill="#fdba74"
            />
          </svg>
          PawBae
        </a>
        <p class="foot-blurb">A desktop companion for humans who work with AI agents.</p>
      </div>
      <div>
        <h3>Features</h3>
        <ul>
          <li><a href="/#features">Agent monitoring</a></li>
          <li><a href="/#visits">Pet visits</a></li>
          <li><a href="/#visits">Shared memories</a></li>
          <li><a href="/#details">OBS stream stage</a></li>
          <li><a href="/#details">Pet diary</a></li>
          <li><a href="/#details">Skin workshop</a></li>
        </ul>
      </div>
      <div>
        <h3>Use Cases</h3>
        <ul>
          <li><a href="/#features">Long-running agent tasks</a></li>
          <li><a href="/#details">Pair-streaming</a></li>
          <li><a href="/#visits">Teammate visits</a></li>
          <li><a href="/#features">Missed-approval rescue</a></li>
        </ul>
      </div>
      <div>
        <h3>Setup Guides</h3>
        <ul>
          <li><a href="/#faq">PawBae + Claude Code</a></li>
          <li><a href="/#faq">PawBae + Cursor</a></li>
          <li><a href="/#faq">PawBae + Codex</a></li>
          <li><a href="/#features">Reading pet moods</a></li>
        </ul>
      </div>
      <div>
        <h3>Compare</h3>
        <ul>
          <li><a href="/#features">vs. menu-bar monitors</a></li>
          <li><a href="/#features">vs. desktop widgets</a></li>
          <li><a href="/#details">vs. stream overlays</a></li>
        </ul>
      </div>
      <div>
        <h3>About</h3>
        <ul>
          <li><a href="https://github.com/PawBae/PawBae-app/blob/main/CHANGELOG.md">Changelog</a></li>
          <li><a href="https://github.com/PawBae">GitHub</a></li>
          <li><a href="/#waitlist">Closed beta</a></li>
        </ul>
      </div>
    </div>
    <div class="foot-legal">
      <span>© 2026 PawBae. All rights reserved.</span>
      <span>pawbae.ai · pawbae.app</span>
    </div>
  </div>
</footer>

<style>
  nav {
    background: var(--hero-bg);
  }
  .nav-in {
    display: flex;
    align-items: center;
    gap: 28px;
    height: 64px;
  }
  .logo {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 18px;
    color: #fff;
    letter-spacing: -0.02em;
    text-decoration: none;
  }
  .nav-links {
    display: flex;
    gap: 24px;
    margin-left: auto;
    font-size: 14.5px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.7);
  }
  .nav-links a {
    text-decoration: none;
    color: inherit;
  }
  .nav-links a:hover,
  .nav-links a:focus-visible {
    color: #fff;
  }
  .nav-cta {
    font-size: 14px;
    padding: 8px 18px;
  }
  @media (max-width: 640px) {
    .nav-links {
      display: none;
    }
    .nav-cta {
      margin-left: auto;
    }
  }

  footer {
    background: #fff;
    border-top: 1px solid #f3f4f6;
    padding: 72px 0 44px;
  }
  .foot-grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 32px 24px;
  }
  @media (max-width: 980px) {
    .foot-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }
  @media (max-width: 620px) {
    .foot-grid {
      grid-template-columns: 1fr 1fr;
    }
  }
  .foot-logo {
    color: var(--ink-900);
  }
  .foot-blurb {
    font-size: 13px;
    color: var(--ink-400);
    max-width: 20em;
    margin: 10px 0 0;
  }
  footer h3 {
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.5;
    color: var(--ink-900);
    margin: 0 0 16px;
  }
  footer ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  footer li a {
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--ink-600);
    text-decoration: none;
  }
  footer li a:hover {
    color: var(--ink-900);
  }
  .foot-legal {
    margin-top: 52px;
    padding-top: 24px;
    border-top: 1px solid #f3f4f6;
    font-size: 12.5px;
    color: var(--ink-400);
    display: flex;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px;
  }
</style>
