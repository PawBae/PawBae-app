<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { untrack } from 'svelte';
  import { _, locale } from 'svelte-i18n';
  import { settingsStore } from '../stores/settings.svelte';
  import { type CustomSkinMeta, skinsStore } from '../stores/skins.svelte';
  import { type CodexPet, DEFAULT_PET_ID } from '../utils/codex-pet';
  import { tryInvoke } from '../utils/invoke';
  import { effectiveName } from '../utils/pet-name';
  import { type ImageDims, type SkinValidation, validateSkin } from '../utils/skin-validate';
  import { petJsonUrlFromSheetUrl, tileFrameStyle } from '../utils/skins';
  import { track } from '../utils/telemetry';

  let { open = false, onclose }: { open?: boolean; onclose: () => void } = $props();

  let busy = $state(false);
  let issues = $state<SkinValidation | null>(null);
  let importedOk = $state(false);
  let confirmRemove = $state<string | null>(null);

  const guideUrl = $derived(
    `https://github.com/PawBae/PawBae-app/blob/main/docs/skins/SKIN-SPEC.${
      $locale?.startsWith('zh') ? 'zh' : 'en'
    }.md`,
  );

  // Rising edge only (ShareCardModal lesson): re-list on every open so folders the
  // user dropped by hand show up, and clear stale result banners.
  let prevOpen = false;
  $effect(() => {
    if (open && !prevOpen) {
      issues = null;
      importedOk = false;
      confirmRemove = null;
      untrack(() => void skinsStore.refresh());
    }
    prevOpen = open;
  });

  async function choose(skin: CodexPet) {
    if (skin.id === settingsStore.miniPetId) return;
    await settingsStore.setMiniPetId(skin.id);
    // Builtin ids are a fixed vocabulary; custom ids are user content — never sent.
    track('skin_switched', { id: skinsStore.customIds.has(skin.id) ? 'custom' : skin.id });
  }

  function imageDims(src: string): Promise<ImageDims | null> {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight });
      img.onerror = () => resolve(null);
      img.src = src;
    });
  }

  /** null = manifest/sheet unreadable (treated as fatal by the caller). */
  async function validateImported(meta: CustomSkinMeta): Promise<SkinValidation | null> {
    try {
      const url = petJsonUrlFromSheetUrl(meta.spritesheetUrl);
      if (!url) return null;
      const res = await fetch(url);
      if (!res.ok) return null;
      const raw: unknown = await res.json();
      const dims = await imageDims(meta.spritesheetUrl);
      if (!dims) return null;
      return validateSkin(raw, dims);
    } catch {
      return null;
    }
  }

  // 严出: the folder is already copied when we validate (the webview can't read
  // arbitrary paths pre-copy), so a fatal report rolls the copy back.
  async function finishImport(meta: CustomSkinMeta, kind: 'folder' | 'image') {
    const report = await validateImported(meta);
    if (!report || report.errors.length > 0) {
      await tryInvoke('remove_custom_skin', { id: meta.id });
      issues = report ?? { errors: [{ key: 'unreadable' }], warnings: [] };
      track('skin_imported', { kind, result: 'invalid' });
      return;
    }
    issues = report.warnings.length > 0 ? report : null;
    importedOk = true;
    skinsStore.noteImportWarnings(meta.id, report.warnings.length);
    await skinsStore.refresh();
    await settingsStore.setMiniPetId(meta.id);
    track('skin_imported', { kind, result: 'ok' });
  }

  async function runImport(kind: 'folder' | 'image') {
    if (busy) return;
    busy = true;
    issues = null;
    importedOk = false;
    try {
      const picked = await invoke<string | null>(
        kind === 'folder' ? 'pick_codex_pet_folder' : 'pick_skin_image',
      );
      if (!picked) return;
      const meta = await invoke<CustomSkinMeta>(
        kind === 'folder' ? 'import_codex_pet' : 'import_skin_image',
        { srcPath: picked },
      );
      await finishImport(meta, kind);
    } catch (e) {
      issues = { errors: [{ key: 'importFailed', params: { reason: String(e) } }], warnings: [] };
      track('skin_imported', { kind, result: 'invalid' });
    } finally {
      busy = false;
    }
  }

  async function removeSkin(id: string) {
    confirmRemove = null;
    await tryInvoke('remove_custom_skin', { id });
    if (settingsStore.miniPetId === id) {
      await settingsStore.setMiniPetId(DEFAULT_PET_ID);
    }
    await skinsStore.refresh();
  }

  function issueText(issue: { key: string; params?: Record<string, string | number> }): string {
    return $_(`skin.issue.${issue.key}`, { values: issue.params });
  }
</script>

{#if open}
  <div class="overlay" role="presentation" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal" onclick={(e) => e.stopPropagation()}>
      <h3 class="title">🎨 {$_('skin.title')}</h3>

      <div class="grid">
        {#each skinsStore.all as skin (skin.id)}
          <div class="tile-wrap">
            <button
              class="tile"
              class:active={skin.id === settingsStore.miniPetId}
              onclick={() => choose(skin)}
              title={skin.description}
            >
              <div class="sprite-crop">
                <div style={tileFrameStyle(skin, 56)}></div>
              </div>
              <span class="tile-name">
                {effectiveName(settingsStore.petNicknames[skin.id], skin.displayName)}
              </span>
              {#if skinsStore.customIds.has(skin.id)}
                <span class="badge">{$_('skin.customBadge')}</span>
              {/if}
              {#if skinsStore.importWarningCount(skin.id) > 0}
                <span class="warn" title={$_('skin.issueWarnings')}>⚠️</span>
              {/if}
            </button>
            {#if skinsStore.customIds.has(skin.id)}
              <button
                class="remove"
                title={$_('skin.remove')}
                onclick={() => (confirmRemove = skin.id)}
              >
                🗑
              </button>
            {/if}
          </div>
        {/each}
      </div>

      {#if confirmRemove !== null}
        <div class="confirm-bar">
          <span>{$_('skin.removeConfirm')}</span>
          <button class="danger" onclick={() => confirmRemove && removeSkin(confirmRemove)}>
            {$_('skin.remove')}
          </button>
          <button onclick={() => (confirmRemove = null)}>{$_('skin.cancel')}</button>
        </div>
      {/if}

      {#if importedOk}
        <p class="ok">{$_('skin.importOk')}</p>
      {/if}
      {#if issues}
        <div class="issues">
          {#if issues.errors.length > 0}
            <p class="issues-head error">{$_('skin.issueErrors')}</p>
            {#each issues.errors as issue}
              <p class="issue error">• {issueText(issue)}</p>
            {/each}
          {/if}
          {#if issues.warnings.length > 0}
            <p class="issues-head">{$_('skin.issueWarnings')}</p>
            {#each issues.warnings as issue}
              <p class="issue">• {issueText(issue)}</p>
            {/each}
          {/if}
        </div>
      {/if}

      <div class="actions">
        <button onclick={() => runImport('folder')} disabled={busy}>
          📁 {$_('skin.importFolder')}
        </button>
        <button onclick={() => runImport('image')} disabled={busy}>
          🖼️ {$_('skin.importImage')}
        </button>
        <button onclick={() => tryInvoke('open_codex_pets_dir')}>
          {$_('skin.openFolder')}
        </button>
        <button onclick={() => tryInvoke('open_url', { url: guideUrl })}>
          {$_('skin.guide')}
        </button>
      </div>

      <button class="close" onclick={onclose}>✕</button>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
  }

  .modal {
    position: relative;
    background: #1a1a20;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 18px 18px 14px;
    width: 340px;
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .title {
    margin: 0;
    color: #fff;
    font-size: 15px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    overflow-y: auto;
    padding: 2px;
  }

  .tile-wrap {
    position: relative;
  }

  .tile {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 2px 6px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    cursor: pointer;
  }

  .tile:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .tile.active {
    border-color: rgba(100, 149, 237, 0.8);
    background: rgba(100, 149, 237, 0.12);
  }

  /* Clip oversized first frames (single-image skins) to a uniform cell. */
  .sprite-crop {
    width: 56px;
    height: 56px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    overflow: hidden;
  }

  .tile-name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: rgba(255, 255, 255, 0.75);
    font-size: 10px;
  }

  .badge {
    position: absolute;
    top: 4px;
    left: 4px;
    padding: 1px 4px;
    border-radius: 6px;
    background: rgba(100, 149, 237, 0.25);
    color: rgba(160, 195, 255, 0.95);
    font-size: 8px;
  }

  .warn {
    position: absolute;
    top: 2px;
    right: 2px;
    font-size: 10px;
  }

  .remove {
    position: absolute;
    right: 2px;
    bottom: 2px;
    padding: 1px 3px;
    background: none;
    border: none;
    font-size: 10px;
    opacity: 0.5;
    cursor: pointer;
  }

  .remove:hover {
    opacity: 1;
  }

  .confirm-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    color: rgba(255, 255, 255, 0.8);
    font-size: 11px;
  }

  .confirm-bar button {
    padding: 3px 8px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 11px;
    cursor: pointer;
  }

  .confirm-bar .danger {
    border-color: rgba(237, 100, 100, 0.5);
    color: rgb(255, 160, 160);
  }

  .ok {
    margin: 0;
    color: rgb(140, 220, 160);
    font-size: 11px;
  }

  .issues {
    max-height: 110px;
    overflow-y: auto;
    padding: 6px 8px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
  }

  .issues-head {
    margin: 0 0 2px;
    color: rgba(255, 255, 255, 0.6);
    font-size: 11px;
    font-weight: 600;
  }

  .issues-head.error {
    color: rgb(255, 140, 140);
  }

  .issue {
    margin: 0;
    color: rgba(255, 255, 255, 0.65);
    font-size: 11px;
    line-height: 1.5;
  }

  .issue.error {
    color: rgba(255, 170, 170, 0.9);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .actions button {
    flex: 1 1 auto;
    padding: 5px 8px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.85);
    font-size: 11px;
    cursor: pointer;
  }

  .actions button:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
  }

  .actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.5);
    font-size: 13px;
    cursor: pointer;
  }

  .close:hover {
    color: #fff;
  }
</style>
