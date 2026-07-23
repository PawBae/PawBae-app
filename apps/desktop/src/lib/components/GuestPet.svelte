<script lang="ts">
  // 访客宠物渲染（line-c W5-6）：props 驱动、无 store 依赖，双宠同屏时由
  // MascotView 传入投影与皮肤。状态映射复用每只宠物自带的 stateMap
  // （petStateToCodexState）——ProjectionStatus 的 idle/working/waiting/compacting
  // 与 MiniPetSourceState 一一对应；offline 没有专属动画行，降级为 idle +
  // 睡意样式（正式 4 只宠物美术到位后换 sleep 行）。
  import type { PublicPetProjection } from '../platform/types';
  import { type CodexPet, petStateToCodexState } from '../utils/codex-pet';
  import SpritePet from './SpritePet.svelte';

  interface Props {
    /** 占位阶段先传内置皮肤；正式版由 projection.skinId 经 skins store 解析 */
    pet: CodexPet;
    projection: PublicPetProjection | null;
    size: number;
    /** 主人归属标签（SV §3.3：来访者保留自己的名字、皮肤和主人归属） */
    ownerHandle?: string;
    showNameTag?: boolean;
    class?: string;
    style?: string;
  }

  let {
    pet,
    projection,
    size,
    ownerHandle,
    showNameTag = true,
    class: className = '',
    style = '',
  }: Props = $props();

  const offline = $derived(projection?.status === 'offline');
  const spriteState = $derived.by(() => {
    const status = projection?.status ?? 'idle';
    return petStateToCodexState(pet, status === 'offline' ? 'idle' : status);
  });
</script>

<div class="guest-pet {className}" {style}>
  {#if showNameTag && projection}
    <div class="name-tag">
      <span class="name">{projection.displayName}</span>
      {#if ownerHandle}<span class="owner">@{ownerHandle}</span>{/if}
    </div>
  {/if}
  <div class="sprite" class:offline>
    <SpritePet {pet} state={spriteState} {size} loop />
    {#if offline}<span class="zzz" aria-hidden="true">zZ</span>{/if}
  </div>
</div>

<style>
  .guest-pet {
    position: relative;
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .name-tag {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    background: rgba(255, 255, 255, 0.9);
    backdrop-filter: blur(8px);
    border: 1px solid rgba(148, 163, 184, 0.25);
    border-radius: 999px;
    padding: 2px 10px;
    box-shadow: 0 4px 14px rgba(30, 41, 59, 0.12);
    white-space: nowrap;
  }
  .name {
    font-size: 11px;
    font-weight: 700;
    color: #334155;
  }
  .owner {
    font-size: 10px;
    color: #94a3b8;
  }
  .sprite {
    position: relative;
    transition: opacity 0.6s ease, filter 0.6s ease;
  }
  /* offline = 主人的 agent 在休息：睡意化而不是「故障化」（never-punish 视觉层） */
  .sprite.offline {
    opacity: 0.75;
    filter: saturate(0.7);
  }
  .zzz {
    position: absolute;
    top: -6px;
    right: -8px;
    font-size: 13px;
  }
</style>
