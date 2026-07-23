// Skin gallery store: builtins (bundled /assets/builtin) + customs (~/.codex/pets
// via the codexpet:// protocol), merged with custom-wins-on-id semantics. This is
// the single lookup every component uses to turn miniPetId into a CodexPet — the
// four copies of "loadCodexPets().find(id) ?? default" used to only know builtins.
// See docs/superpowers/specs/2026-07-09-skin-workshop-design.md §2.1.

import {
  type CodexPet,
  DEFAULT_PET_ID,
  loadCodexPets,
  type RawPetMeta,
  resolvePet,
} from '../utils/codex-pet';
import { tryInvoke } from '../utils/invoke';
import { mergeSkins, petJsonUrlFromSheetUrl } from '../utils/skins';

/** Wire shape of Rust's list_custom_codex_pets / import commands. */
export interface CustomSkinMeta {
  id: string;
  displayName: string;
  description: string;
  spritesheetUrl: string;
  /** Folder-root pet.json URL (staged imports live under .staging/<id>). */
  petJsonUrl?: string;
}

async function loadCustomSkins(): Promise<CodexPet[]> {
  const metas = await tryInvoke<CustomSkinMeta[]>('list_custom_codex_pets');
  if (!Array.isArray(metas)) return [];
  const out = await Promise.all(
    metas.map(async (m): Promise<CodexPet | null> => {
      try {
        const url = m.petJsonUrl ?? petJsonUrlFromSheetUrl(m.spritesheetUrl);
        if (!url) return null;
        const res = await fetch(url);
        if (!res.ok) return null;
        const raw = (await res.json()) as RawPetMeta;
        return resolvePet(raw, m.id, m.spritesheetUrl);
      } catch {
        return null;
      }
    }),
  );
  return out.filter((p): p is CodexPet => p !== null);
}

class SkinsStore {
  all = $state<CodexPet[]>([]);
  customIds = $state<ReadonlySet<string>>(new Set());
  /**
   * Bumped on every refresh. Effects that hold a resolved pet must track this
   * alongside miniPetId — re-importing a skin upgrades it in place without
   * changing the id, and the id alone would never re-fire them.
   */
  revision = $state(0);
  /** Session-only import warnings, id → count (gallery tile badge). */
  private importWarnings = $state<Record<string, number>>({});

  private loaded: Promise<void> | null = null;

  /** Idempotent first load — Main.init calls this; refresh() forces a reload. */
  ensureLoaded(): Promise<void> {
    if (!this.loaded) this.loaded = this.refresh();
    return this.loaded;
  }

  async refresh(): Promise<void> {
    const [builtins, customs] = await Promise.all([loadCodexPets(), loadCustomSkins()]);
    this.customIds = new Set(customs.map((p) => p.id));
    this.all = mergeSkins(builtins, customs);
    this.revision += 1;
  }

  /** The skin to render for a wanted id: exact match, else Yoonie, else the first. */
  resolve(id: string | null | undefined): CodexPet | null {
    const list = this.all;
    if (list.length === 0) return null;
    return list.find((p) => p.id === id) ?? list.find((p) => p.id === DEFAULT_PET_ID) ?? list[0];
  }

  /** Exact identity lookup for remote/official pets. Never aliases an unfinished id to Yoonie. */
  resolveExact(id: string | null | undefined): CodexPet | null {
    if (!id) return null;
    return this.all.find((pet) => pet.id === id) ?? null;
  }

  noteImportWarnings(id: string, count: number): void {
    this.importWarnings = { ...this.importWarnings, [id]: count };
  }

  importWarningCount(id: string): number {
    return this.importWarnings[id] ?? 0;
  }
}

export const skinsStore = new SkinsStore();
