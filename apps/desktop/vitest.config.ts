import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: { alias: { '$lib': resolve(__dirname, 'src/lib') } },
  // Scope to src/: agent worktrees under .claude/worktrees/ carry their own copies
  // of the suite, and vitest's default glob would sweep them into every local run.
  test: { environment: 'jsdom', include: ['src/**/*.test.ts'] }
});
