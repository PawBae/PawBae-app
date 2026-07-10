import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  resolve: { alias: { "$lib": resolve(__dirname, "src/lib") } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
