# Changelog

## v0.2.0 — growth update

### Evolution
- Your pet now grows through five stages — Newborn → Sprout → Junior →
  Master → Legend — fueled by lifetime coins earned from real work
  (agent completions, focus, gifts). Spending never causes regression.
- From Junior on it branches by your working style: Commander
  (agent-driven, blue aura), Zen (focus/pomodoro, green aura) or
  Companion (nurture, pink aura). Legend gets a pulsing glow.
- Evolution moments play a radial flash + announcement bubble.

### Achievements
- 24 achievements across agent tasks, focus blocks, typing volume,
  feeding, gifts, streaks, days together and evolution — a few hidden.
  Achievement wall in the pet panel; unlocks pop a celebration toast.

### Daily gift & streaks
- The daily gift now has a claim button in the pet panel.
- Consecutive-day streaks raise the payout (50 → 80 coins, capped at
  7 days). Breaking a streak just restarts it — no punishment. The
  panel shows your streak flame and "Day N together".

### Agent-aware animations
- The mascot mirrors live agent state — working, compacting, and
  waiting-for-you — not just busy/idle, via each pet's own rows.
- A short status bubble in coding mode: "Needs your eyes" / "Tidying
  context" / "On it…". Waiting and compacting persist; working only
  flashes briefly so it never becomes a log window.
- 3+ agents busy in parallel makes the pet tremble with overload.

### Richer idle behavior
- While idle the pet plays short personality actions from its own
  sheet (yoonie blinks/pounces; others wave), on the long-dormant
  "Random Action Interval" setting.
- Time-of-day bias: calm at night, lively at midday.

### Control & fixes
- Move the pet with WASD / arrow keys while the mini window is focused;
  Shift to move faster.
- Fixed: a strolling pet no longer drags the expanded panel around the
  screen.

## v0.1.0 — first public release

### Desktop pet
- Sprite-animated companion with physics: drag, throw, wall-grab, walk, stroll mode
- Pet life system: hunger, affection, coins, feeding, daily gift, Pomodoro timer
- Input reactions (macOS): the pet reacts to typing and clicking — privacy-safe
  (event counts only) and switchable under Settings → Privacy
- Multiple built-in pets plus custom Codex-pet import

### Agent monitoring (Coding Mode)
- Claude Code, Codex, and Cursor status in real time via hooks: thinking,
  running tools, compacting, waiting for approval, done
- Session panel with conversation previews, token stats, and 14-day usage charts
- Completion/waiting sounds with per-agent toggles
- Click-to-jump to the agent's terminal (macOS)
- Remote OpenClaw monitoring over SSH

### App
- macOS (Apple Silicon + Intel) and Windows 10/11
- English and Chinese UI
- In-app auto-update with startup prompt and skip-version support
