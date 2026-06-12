#!/bin/zsh
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is not installed."
  echo "Install it first, then run this file again:"
  echo "  corepack enable"
  echo "  corepack prepare pnpm@latest --activate"
  echo
  read -r "?Press Enter to close..."
  exit 1
fi

if [ ! -d node_modules ]; then
  echo "Installing dependencies..."
  pnpm install
fi

echo "Starting PawBae..."
pnpm tauri dev
