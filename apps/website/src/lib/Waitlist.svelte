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
      // 走 RPC 而非表直连：waitlist 表的直接读写已全部 revoke（PR #54 云端
      // schema），join_waitlist 恒定无返回——重复报名与新报名同形，不可探测。
      const res = await fetch(`${supabaseUrl}/rest/v1/rpc/join_waitlist`, {
        method: 'POST',
        headers: {
          apikey: supabaseAnonKey,
          Authorization: `Bearer ${supabaseAnonKey}`,
          'Content-Type': 'application/json',
          Prefer: 'return=minimal'
        },
        body: JSON.stringify({ p_email: email.trim().toLowerCase() })
      });
      status = res.ok ? 'done' : 'error';
    } catch {
      status = 'error';
    }
  }
</script>

{#if configured}
  {#if status === 'done'}
    <p class="wait-done">You're on the list! Your pet will fetch you an invite. 🐾</p>
  {:else}
    <form class="wait-form" aria-label="waitlist" onsubmit={submit}>
      <input
        type="email"
        placeholder="you@example.com"
        aria-label="email address"
        bind:value={email}
        required
      />
      <button class="btn btn-yellow" type="submit" disabled={status === 'sending'}>
        {status === 'sending' ? 'Sending…' : 'Join the waitlist'}
      </button>
    </form>
    {#if status === 'error'}
      <p class="wait-error">That didn't go through — mind trying again in a moment?</p>
    {/if}
  {/if}
{:else}
  <p class="wait-soon">The waitlist opens with the closed beta — check back soon.</p>
{/if}

<style>
  .wait-form {
    display: flex;
    gap: 12px;
    margin-top: 28px;
    max-width: 460px;
  }
  .wait-form input {
    flex: 1;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    padding: 11px 16px;
    font-size: 15px;
    font-family: inherit;
    color: #fff;
  }
  .wait-form input::placeholder {
    color: #6b7280;
  }
  .wait-form input:focus-visible {
    outline: 3px solid var(--primary);
    border-color: var(--primary);
  }
  @media (max-width: 520px) {
    .wait-form {
      flex-direction: column;
    }
  }
  .wait-form button[disabled] {
    opacity: 0.7;
    cursor: default;
  }
  .wait-done {
    margin-top: 28px;
    font-size: 16px;
    font-weight: 600;
    color: #fff;
  }
  .wait-error {
    margin-top: 12px;
    font-size: 13.5px;
    color: #fca5a5;
  }
  .wait-soon {
    margin-top: 28px;
    font-size: 15px;
    color: var(--dark-body);
  }
</style>
