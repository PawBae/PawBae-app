<script lang="ts">
  import { env } from '$env/dynamic/public';

  // 候补名单红线（发布计划 §3 / line-c W3-4）：公开上线那一刻必须真实收集，
  // 禁止 no-op。env 未配置时诚实展示「即将开放」，绝不做假装成功的表单。
  const supabaseUrl = env.PUBLIC_SUPABASE_URL ?? '';
  const supabaseAnonKey = env.PUBLIC_SUPABASE_ANON_KEY ?? '';
  const configured = Boolean(supabaseUrl && supabaseAnonKey);

  let email = $state('');
  let status = $state<'idle' | 'sending' | 'done' | 'error'>('idle');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (status === 'sending') return;
    status = 'sending';
    try {
      const res = await fetch(`${supabaseUrl}/rest/v1/waitlist`, {
        method: 'POST',
        headers: {
          apikey: supabaseAnonKey,
          Authorization: `Bearer ${supabaseAnonKey}`,
          'Content-Type': 'application/json',
          Prefer: 'return=minimal'
        },
        body: JSON.stringify({ email: email.trim().toLowerCase() })
      });
      // 409 = 邮箱已在名单里，对用户来说同样是成功
      status = res.ok || res.status === 409 ? 'done' : 'error';
    } catch {
      status = 'error';
    }
  }
</script>

{#if configured}
  {#if status === 'done'}
    <div class="wait-feedback wait-done" role="status">
      <span class="feedback-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24">
          <path
            d="m6 12 4 4 8-9"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </span>
      <span><b>You're on the list.</b> Bobo will fetch you an invite.</span>
    </div>
  {:else}
    <form class="wait-form" aria-label="waitlist" onsubmit={submit}>
      <label class="sr-only" for="waitlist-email">Email address</label>
      <div class="input-wrap">
        <span class="input-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24">
            <path
              d="M4 7h16v11H4zm0 1 8 6 8-6"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linejoin="round"
            />
          </svg>
        </span>
        <input
          id="waitlist-email"
          name="email"
          type="email"
          placeholder="you@example.com"
          aria-label="email address"
          aria-describedby={status === 'error' ? 'waitlist-error' : undefined}
          autocomplete="email"
          bind:value={email}
          required
        />
      </div>
      <button class="btn btn-yellow" type="submit" disabled={status === 'sending'}>
        {status === 'sending' ? 'Sending...' : 'Join the waitlist'}
      </button>
    </form>
    {#if status === 'error'}
      <p class="wait-error" id="waitlist-error" role="alert">
        That did not go through. Please try again in a moment.
      </p>
    {/if}
  {/if}
{:else}
  <div class="wait-feedback wait-soon" role="status">
    <span class="feedback-icon" aria-hidden="true">
      <svg viewBox="0 0 24 24">
        <path
          d="M12 7v5l3 2m5-2a8 8 0 1 1-16 0 8 8 0 0 1 16 0Z"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </span>
    <span><b>Invites open soon.</b> Check back with the next closed beta wave.</span>
  </div>
{/if}

<style>
  .wait-form {
    display: flex;
    max-width: 34rem;
    gap: 0.75rem;
    margin-top: 1.75rem;
  }

  .input-wrap {
    position: relative;
    min-width: 0;
    flex: 1;
  }

  .input-icon {
    position: absolute;
    top: 50%;
    left: 1rem;
    width: 1.1rem;
    height: 1.1rem;
    color: #a8b0bd;
    pointer-events: none;
    transform: translateY(-50%);
  }

  .input-icon svg {
    width: 100%;
    height: 100%;
  }

  .wait-form input {
    width: 100%;
    min-height: 3rem;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: var(--radius-button);
    padding: 0.75rem 1rem 0.75rem 2.8rem;
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
    font-size: 0.94rem;
    transition:
      border-color 180ms ease,
      background-color 180ms ease,
      box-shadow 180ms ease;
  }

  .wait-form input::placeholder {
    color: #a8b0bd;
  }

  .wait-form input:focus-visible {
    border-color: #7dd3fc;
    outline: 3px solid rgba(125, 211, 252, 0.32);
    outline-offset: 2px;
    background: rgba(255, 255, 255, 0.1);
  }

  .wait-form button {
    flex: none;
  }

  .wait-form button[disabled] {
    cursor: wait;
    opacity: 0.68;
  }

  .wait-feedback {
    display: flex;
    max-width: 34rem;
    align-items: center;
    gap: 0.8rem;
    margin-top: 1.75rem;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: var(--radius-card);
    padding: 0.9rem 1rem;
    background: rgba(255, 255, 255, 0.06);
    color: #d7e2f2;
    font-size: 0.9rem;
  }

  .wait-feedback b {
    color: #fff;
  }

  .feedback-icon {
    display: grid;
    width: 2rem;
    height: 2rem;
    flex: none;
    place-items: center;
    border-radius: 999px;
  }

  .feedback-icon svg {
    width: 1rem;
    height: 1rem;
  }

  .wait-done .feedback-icon {
    background: rgba(52, 211, 153, 0.14);
    color: #6ee7b7;
  }

  .wait-soon .feedback-icon {
    background: rgba(125, 211, 252, 0.12);
    color: #bae6fd;
  }

  .wait-error {
    max-width: 34rem;
    margin: 0.75rem 0 0;
    color: #fecaca;
    font-size: 0.82rem;
  }

  @media (max-width: 560px) {
    .wait-form {
      align-items: stretch;
      flex-direction: column;
    }

    .wait-form button {
      width: 100%;
    }
  }
</style>
