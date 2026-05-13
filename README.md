<p align="center">
  <img src="src-tauri/icons/icon.png" width="80" />
</p>
<h1 align="center">PawBae</h1>
<p align="center">
  <b>English</b> | <a href="./README.zh.md">中文</a>
</p>
<p align="center">
  An AI-powered desktop pet that monitors your coding agents in real time.
</p>

## What it does

- **Desktop pet** — a sprite-animated companion that lives on your screen
- **Agent monitoring** — reacts to Claude Code, Codex, and Cursor activity in real time (working, idle, waiting)
- **Message bubbles** — displays agent status notifications and AI conversation replies
- **Interactive** — click, drag, and interact with your pet
- **Pet life system** — hunger, affection, coins, feeding, and Pomodoro timer

## Tech Stack

- **Frontend**: Svelte 5 + TypeScript + Vite
- **Desktop**: Tauri 2 (Rust)
- **Styling**: Tailwind CSS

## Requirements

- macOS or Windows
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Codex](https://github.com/openai/codex), and/or [Cursor](https://www.cursor.com) installed (for agent monitoring)

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

## License

MIT
