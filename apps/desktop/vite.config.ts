import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "safari16", sourcemap: false },
  resolve: { conditions: ["browser"] },
  test: {
    environment: "happy-dom",
    include: ["tests/**/*.test.ts"],
  },
});
