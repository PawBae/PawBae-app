# Approval Note (叼来审批单) — Design

**Date:** 2026-07-08
**Feature:** When an agent session waits on the user (permission / question), the pet
presents a clickable "approval note"; clicking focuses the waiting terminal; a fast
response earns affection.
**Roadmap ref:** Phase 1 「叼来审批单 [S]」, per the approved strategy's core loop
(response latency is where the pet earns its keep) and its red lines.
**Owner decision (2026-07-08):** clickable note bubble form — the S-sized tier; the
"note in mouth" sprite overlay and walk-to-cursor tiers were deliberately deferred.

## Goal

Waiting agents currently show a passive 👀 bubble the user can't act on. Turn that
moment into the pet's job: it visibly brings you the approval slip, one click puts you
in the right terminal, and answering promptly is rewarded — the pet becomes the
fastest path back to a blocked agent instead of a bystander.

**Red lines (from the approved strategy):** never punish — a slow or absent response
does nothing, the note just waits quietly; never interrupt — no focus stealing, no OS
notifications, the note lives in the pet's own window; reward the outcome, not the
click path — answering directly in the terminal earns the same affection.

## Approaches considered

1. **Upgrade AgentBubble's waiting variant in place.** Rejected: AgentBubble is
   deliberately `pointer-events: none` and multiplexes three transient states; making
   one variant interactive complicates a stable component.
2. **New ApprovalNote component + suppress AgentBubble while it shows — CHOSEN.**
   The existing `suppressed` prop already models "something outranks me". Waiting gets
   its own clickable surface; compacting/working bubbles are untouched.
3. **Rust-side push of waiting sessions with richer metadata.** Rejected: the 2s-polled
   session list already carries `status === 'waiting'` per session; zero Rust changes
   needed (`jump_to_claude_terminal` / `focus_cursor_terminal` already exist).

## Mechanics

- **Trigger:** any session in `sessionStore.claudeSessions` with `status === 'waiting'`
  (coding mode only — pet mode has no sessions). Note appears on the rising edge,
  shows a count suffix ` (N)` when several wait (AgentBubble precedent), and
  disappears when no session waits.
- **Click:** focuses the **longest-waiting** session's terminal (deterministic, no
  picker): `focus_cursor_terminal` for `source === 'cursor'`, else
  `jump_to_claude_terminal`. Failure logs a warning; the note stays.
- **Affection:** a session leaving `waiting` within **2 minutes** of its note
  appearing earns **+2 affection** (headpat scale), capped at **10 awards/day**
  (ephemeral daily counter, same as headpat's). Awarded regardless of whether the
  user clicked the note or responded directly in the terminal. Any exit from waiting
  counts as a response (approve, deny, interrupt — all are the user acting).
- **Pet beat:** yoonie's `stateMap.waiting` changes `hide` → `asking-user` (row 17,
  already in its sheet). A pet that hides while asking for approval contradicts the
  feature's story; other skins keep their own waiting mappings.

## Components

| unit | responsibility |
|------|----------------|
| `src/lib/utils/approval-note.ts` (new) | Pure machine: pending map (sessionId → firstSeen ms, insertion order = age), `stepApprovalNotes(state, waitingIds, now)` → cleared responses with waited ms, `oldestPending`, `approvalAwardFor(waitedMs, todayCount)`. Zero Svelte/Tauri imports. |
| `src/lib/utils/approval-note.test.ts` (new) | Rising/falling edges, timestamp stability across polls, multi-session ordering, fast/slow boundary, daily cap, corrupt input. |
| `src/lib/components/ApprovalNote.svelte` (new) | Paper-note styled clickable bubble (button semantics + aria-label), count suffix, urgent pulse. Pure presentation; click handler injected. |
| `src/lib/stores/pet.svelte.ts` | `applyApprovalResponse(waitedMs)`: award decision via `approvalAwardFor`, affection commit, ephemeral daily counter fields. |
| `src/lib/stores/pet-approval.test.ts` (new) | Store glue: award, cap, affection clamp. |
| `src/lib/components/MascotView.svelte` | Derives waiting ids (keyed dedupe so the 2s poll's fresh arrays don't re-fire), steps the machine, renders ApprovalNote, suppresses AgentBubble while the note shows, click → jump invoke. |
| `src/lib/i18n/{en,zh}.json` | `approvalNote.label` (+ existing-suffix pattern). |
| `public/assets/builtin/yoonie/pet.json` | `stateMap.waiting: "asking-user"`. |

## Data flow

```
2s session poll (existing) → claudeSessions[].status
  └─ MascotView: waitingIds (coding mode) — keyed against join(',') to dedupe polls
       ├─ render: waitingIds.length > 0 → <ApprovalNote count onClick>
       │    └─ onClick → oldestPending(machine) → tryInvoke(jump/focus by source)
       └─ $effect: stepApprovalNotes(machine, waitingIds, Date.now())
            └─ responses → petStore.applyApprovalResponse(waitedMs)
                 └─ approvalAwardFor(waitedMs, todayCount) → +2 affection | no-op
```

## Error handling

- Jump invoke failure → `tryInvoke` logs, note stays (user can still respond manually).
- Clicked session vanished between render and click → machine already pruned it; falls
  back to the next-oldest, no-op if none.
- Non-finite timestamps / negative elapsed → machine returns no responses.

## Testing

Pure machine gets exhaustive vitest (rewards.ts precedent); store glue mirrors
pet-feed.test.ts. Manual verification: inject `PermissionRequest` (→ waiting) then
`UserPromptSubmit` (→ processing) over the hook socket; confirm note appears with
asking-user beat, click focuses the terminal, fast clear awards +2 affection once,
and a >2min clear awards nothing.

## Out of scope (deliberately)

Note-in-mouth sprite overlay and walk-to-cursor (deferred tiers), per-session note
stacking UI, response-time analytics, sound on note appearance (never-interrupt says
default silent; revisit with the sound settings section), Windows-specific terminal
focus differences (jump command's existing behavior is what it is).

## Addendum — what live click testing surfaced (2026-07-08, second pass)

The owner's real clicks (and a long adversarial session of synthetic ones) found four
real defects the unit suite could not see, all fixed on this branch:

1. **Bubbles clipped at the bottom edge.** Coding mode forced `bubbleAbove = false`,
   so every bubble rendered below the pet and clipped to a sliver whenever the pet
   walked the screen's bottom edge — its usual habitat. The placement poll now runs in
   both modes and ApprovalNote takes a `placement` prop (CelebrationBubble idiom).
2. **Collapsed-mode DOM clicks are not a thing on macOS.** The mini window is a
   non-key floating panel: presses land in `pet_core`'s NSEvent machinery (that is why
   hover and drag are Rust-driven). The note press was being consumed as a drag-start.
   Fix: `pet_core` detects presses inside the note strip (fed by the new
   `set_note_hitbox` command + `PetState` atomics) and emits `approval-note-click`;
   MascotView listens and jumps. The DOM button stays for Windows (fully interactive
   window) and accessibility. Note taps outrank drag-engage where the zones overlap.
3. **A bare `waitingKey;` statement is not a tracked read.** The step effect never
   re-fired, so the approval machine stayed empty and a visible note had no session to
   jump to. Ids now derive from `waitingKey` inside the effect; `respondToApproval`
   also falls back to the live store's first waiting session.
4. **`ClaudeSession.id` never existed on the wire.** Rust serializes
   `session_id → sessionId`; the TS interface said `id`, so consumers read undefined —
   including the pre-existing Panel session list (keys and click-to-select). Interface
   and all consumers now use `sessionId`.

Diagnostics kept on purpose: `[approval-note] native tap` (pet_core) and the frontend
`jump via …` debug_log — one line on each side of the native→webview hop.
