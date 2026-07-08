# Token Feeding Loop — Design

**Date:** 2026-07-07
**Feature:** Completed agent tasks feed the pet; token spend sizes the meal.
**Roadmap ref:** ROADMAP.md §2.2 (Token 花费感知化), revised per the approved strategy's
care-inversion loop ("干活就是养宠") and its no-punishment red line.

## Goal

When a coding agent (CC / Codex / Cursor) genuinely completes a task, the pet visibly
eats: hunger is restored by an amount derived from the tokens that task burned
("便宜操作是小零食,贵操作是大餐"). Agent work becomes the literal act of pet care —
the retention loop IS the adoption loop.

**Design red line (from the approved strategy):** never punish. Token burn must NOT
drain hunger; idle days leave the existing slow decay untouched (the pet naps, it
doesn't starve). This deliberately revises ROADMAP §2.2's original "每消耗 token,
食物缓慢减少" framing, which is a guilt loop.

## Approaches considered

1. **Rust-side: extend the `claude-task-complete` payload with token totals.**
   Rejected: requires per-session usage accounting at Stop time in Rust, duplicating
   what `get_claude_stats` already does; largest diff, hardest to test.
2. **Frontend watermark + `get_claude_stats` on completion — CHOSEN.**
   The command already parses per-source JSONL usage (input/output/cache tokens).
   On each completion event we fetch totals for that source and settle the delta
   against an in-memory baseline. Zero Rust changes; pure-logic reducer is unit-testable
   in the established rewards.ts style.
3. **Continuous metrics polling that drains hunger by burn (ROADMAP §2.2 original).**
   Rejected: punishment loop (violates red line) + the cumulative-total double-count
   trap ROADMAP itself warns about + a new poll loop for no benefit.

## Mechanics

- **Nutrition** = `totalInputTokens + totalOutputTokens` per source. Cache read/write
  tokens are excluded: cache reads are cheap and would inflate every meal to a feast.
- **Meal tiers** (delta of nutrition since the last meal for that source):
  | tier  | min delta | hunger restore |
  |-------|-----------|----------------|
  | snack | ≥ 2,000   | +5             |
  | meal  | ≥ 60,000  | +12            |
  | feast | ≥ 300,000 | +20            |
- **Crumbs carry:** a delta under 2,000 does NOT move the baseline — tiny turns
  accumulate until they add up to a snack.
- **Baselines are ephemeral** (per app run), matching hunger itself, and are primed
  at store init via one best-effort `get_claude_stats` per source. If priming failed,
  the first settle sets the baseline and feeds nothing (prevents a giant lifetime-total
  feast on first sight). A shrinking counter (deleted session files) re-baselines
  silently and feeds nothing — never a negative meal.
- **Free food:** the agent brought dinner home. No coins are spent or minted (the
  coin economy already pays `agent_stop` separately); the ledger is untouched.
- **Affection bonus:** mirrors `applyFeed` — if hunger was < 30 when the meal lands,
  `AFFECTION_FEED_HUNGRY` (+5) affection.
- **Reaction:** `currentAction = 'eat'`, same slot/revert machinery manual feeding uses.
- **Waiting events feed nothing:** `waiting: true` is a permission pause, not a
  completion; the settle isn't invoked at all.
- **Busy lock** on the async settle (per CLAUDE.md polling lessons): a completion
  arriving while a previous `get_claude_stats` is in flight is skipped; its tokens
  are not lost — they stay in the delta for the next completion.

## Components

| unit | responsibility |
|------|----------------|
| `src/lib/utils/token-feed.ts` (new) | Pure reducer: baselines, `primeTokenBaseline`, `settleTokenMeal`, `nutritionOf`, tier constants. Zero Svelte/Tauri imports. |
| `src/lib/utils/token-feed.test.ts` (new) | Unit tests: priming, first-sight fallback, crumb carry, three tiers, counter reset, corrupt input. |
| `src/lib/types.ts` | Add `ClaudeStats` TS interface (mirror of the Rust serde renames actually consumed). |
| `src/lib/stores/pet.svelte.ts` | `applyTokenMeal(meal)` (hunger/affection/eat-action), `settleTokenFeed(source)` (invoke + settle + busy lock), init-time baseline priming, hook into `handleTaskComplete`. |
| `src/lib/stores/pet-feed.test.ts` | Store-glue tests for `applyTokenMeal` (free, clamped, affection gate, eat action). |
| `ROADMAP.md` §2.2 | Reword to the care-inversion framing; mark implemented scope. |

## Data flow

```
Rust hook pipeline (already filters subagent stops / ESC / compaction)
  └─ emits claude-task-complete {sessionId, waiting, source}
       └─ pet.svelte.ts handleTaskComplete
            ├─ awardAgentStop (existing coin path, unchanged)
            └─ if !waiting → settleTokenFeed(source)   [busy-locked]
                 ├─ tryInvoke get_claude_stats(source) → ClaudeStats
                 ├─ settleTokenMeal(state, source, nutritionOf(stats)) → TokenMeal | null
                 └─ meal → applyTokenMeal: hunger+restore (clamp 100),
                           affection bonus if was hungry, action 'eat'
```

## Error handling

- `get_claude_stats` failure → `tryInvoke` logs a warning, settle is a no-op.
- Non-finite / negative nutrition → reducer returns null, state untouched.
- Corrupt baseline can't occur (in-memory only, always written from validated input).

## Testing

Pure reducer gets exhaustive vitest coverage (matches `rewards.test.ts` precedent);
store glue gets the `pet-feed.test.ts` harness (mocked plugin-store/event/core).
Manual verification: run the app, complete a real CC task, watch the eat beat and
hunger bar move.

## Out of scope (deliberately)

Daily token budget alerts (ROADMAP §2.2's second half), per-meal speech bubbles,
souvenir drops (Phase-1 冒险 feature), persistence of baselines, settings toggle
(core loop, always on — manual feeding already has no toggle).
