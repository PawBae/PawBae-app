<script lang="ts">
  import '../app.css';

  let { children } = $props();
</script>

<svelte:head>
  <title>PawBae | The desktop pet for your AI agents</title>
  <meta
    name="description"
    content="PawBae is a tiny desktop companion that works with Claude Code, Codex, and Cursor. See what your agent is doing at a glance."
  />
</svelte:head>

<a class="skip-link" href="#download">Skip to content</a>

<nav class="site-nav" aria-label="Primary navigation">
  <div class="wrap nav-inner">
    <a class="logo" href="/" aria-label="PawBae home">
      <svg class="brand-mark" viewBox="0 0 36 36" aria-hidden="true">
        <rect x="2" y="2" width="32" height="32" rx="10" fill="#0a0a08" />
        <path d="M10 16 9 8l7 5m10 3 1-8-7 5" fill="#7dd3fc" />
        <ellipse cx="18" cy="21" rx="11" ry="9" fill="#ffe3c2" />
        <path
          d="M27 24c5 0 5-4 3-6"
          fill="none"
          stroke="#fb923c"
          stroke-width="3"
          stroke-linecap="round"
        />
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

<main>
  {@render children()}
</main>

<footer>
  <div class="wrap">
    <div class="footer-grid">
      <div class="brand-col">
        <a class="logo footer-logo" href="/" aria-label="PawBae home">
          <svg class="brand-mark" viewBox="0 0 36 36" aria-hidden="true">
            <rect x="2" y="2" width="32" height="32" rx="10" fill="#0a0a08" />
            <path d="M10 16 9 8l7 5m10 3 1-8-7 5" fill="#7dd3fc" />
            <ellipse cx="18" cy="21" rx="11" ry="9" fill="#ffe3c2" />
            <path
              d="M27 24c5 0 5-4 3-6"
              fill="none"
              stroke="#fb923c"
              stroke-width="3"
              stroke-linecap="round"
            />
            <circle cx="14" cy="20" r="1.4" fill="#734b2e" />
            <circle cx="22" cy="20" r="1.4" fill="#734b2e" />
          </svg>
          <span>PawBae</span>
        </a>
        <p class="footer-blurb">A desktop companion for humans who build with AI agents.</p>
      </div>

      <div>
        <h2>Features</h2>
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
        <h2>Use Cases</h2>
        <ul>
          <li><a href="/#features">Long-running agent tasks</a></li>
          <li><a href="/#details">Pair-streaming</a></li>
          <li><a href="/#visits">Teammate visits</a></li>
          <li><a href="/#features">Missed-approval rescue</a></li>
        </ul>
      </div>

      <div>
        <h2>Setup Guides</h2>
        <ul>
          <li><a href="/#faq">PawBae + Claude Code</a></li>
          <li><a href="/#faq">PawBae + Cursor</a></li>
          <li><a href="/#faq">PawBae + Codex</a></li>
          <li><a href="/#features">Reading pet moods</a></li>
        </ul>
      </div>

      <div>
        <h2>Compare</h2>
        <ul>
          <li><a href="/#features">Menu-bar monitors</a></li>
          <li><a href="/#features">Desktop widgets</a></li>
          <li><a href="/#details">Stream overlays</a></li>
        </ul>
      </div>

      <div>
        <h2>About</h2>
        <ul>
          <li><a href="https://github.com/PawBae/PawBae-app/blob/main/CHANGELOG.md">Changelog</a></li>
          <li><a href="https://github.com/PawBae">GitHub</a></li>
          <li><a href="/#waitlist">Closed beta</a></li>
        </ul>
      </div>
    </div>

    <div class="footer-legal">
      <span>© 2026 PawBae. All rights reserved.</span>
      <span>pawbae.ai · pawbae.app</span>
    </div>
  </div>
</footer>

<style>
  .skip-link {
    position: fixed;
    top: 0.75rem;
    left: 0.75rem;
    z-index: 40;
    border-radius: 0.6rem;
    padding: 0.65rem 0.9rem;
    background: var(--primary);
    color: #fff;
    font-size: 0.875rem;
    font-weight: 700;
    text-decoration: none;
    transform: translateY(-160%);
    transition: transform 160ms ease;
  }

  .skip-link:focus {
    transform: translateY(0);
  }

  .site-nav {
    position: relative;
    z-index: 20;
    border-bottom: 1px solid rgba(226, 232, 240, 0.7);
    background: rgba(255, 255, 255, 0.9);
    -webkit-backdrop-filter: blur(18px) saturate(150%);
    backdrop-filter: blur(18px) saturate(150%);
  }

  .nav-inner {
    display: flex;
    height: 71px;
    align-items: center;
    gap: 1.75rem;
  }

  .logo {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 0.65rem;
    color: var(--ink-950);
    font-size: 1.125rem;
    font-weight: 760;
    letter-spacing: -0.03em;
    text-decoration: none;
  }

  .brand-mark {
    width: 2rem;
    height: 2rem;
  }

  .nav-links {
    display: flex;
    align-items: center;
    gap: 1.6rem;
    margin-left: auto;
    color: var(--ink-600);
    font-size: 0.9rem;
    font-weight: 600;
  }

  .nav-links a {
    text-decoration: none;
    transition: color 160ms ease;
  }

  .nav-links a:hover,
  .nav-links a:focus-visible {
    color: var(--ink-950);
  }

  .nav-cta {
    min-height: 2.75rem;
    padding: 0.6rem 1.2rem;
    font-size: 0.9rem;
  }

  footer {
    border-top: 1px solid #f1f5f9;
    background: #fff;
    padding: clamp(4.5rem, 8vw, 6rem) 0 2.75rem;
  }

  .footer-grid {
    display: grid;
    grid-template-columns: minmax(180px, 1.4fr) repeat(5, minmax(0, 1fr));
    gap: 2.5rem 1.5rem;
  }

  .footer-logo {
    width: fit-content;
  }

  .footer-blurb {
    max-width: 19rem;
    margin: 0.9rem 0 0;
    color: var(--ink-500);
    font-size: 0.875rem;
    line-height: 1.65;
  }

  footer h2 {
    margin: 0 0 1rem;
    color: var(--ink-900);
    font-size: 0.875rem;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  footer ul {
    display: grid;
    gap: 0.65rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  footer li a {
    color: var(--ink-600);
    font-size: 0.84rem;
    line-height: 1.45;
    text-decoration: none;
    transition: color 160ms ease;
  }

  footer li a:hover,
  footer li a:focus-visible {
    color: var(--ink-950);
  }

  .footer-legal {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 0.75rem 1.5rem;
    margin-top: 3.75rem;
    border-top: 1px solid #f1f5f9;
    padding-top: 1.5rem;
    color: var(--ink-500);
    font-size: 0.78rem;
  }

  @media (max-width: 980px) {
    .footer-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }

  @media (max-width: 680px) {
    .nav-links {
      display: none;
    }

    .nav-cta {
      margin-left: auto;
    }
  }

  @media (max-width: 620px) {
    .nav-inner {
      height: 63px;
      gap: 1rem;
    }

    .logo {
      font-size: 1.05rem;
    }

    .brand-mark {
      width: 1.85rem;
      height: 1.85rem;
    }

    .nav-cta {
      min-height: 2.5rem;
      padding-inline: 0.9rem;
      font-size: 0.84rem;
    }

    .footer-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .brand-col {
      grid-column: 1 / -1;
    }

    .footer-legal {
      flex-direction: column;
    }
  }

  @media (prefers-reduced-transparency: reduce) {
    .site-nav {
      background: #fff;
      -webkit-backdrop-filter: none;
      backdrop-filter: none;
    }
  }
</style>
