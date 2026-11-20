import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'node:path';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      input: {
        tick: resolve(import.meta.dirname, 'tick.html'),
        popover: resolve(import.meta.dirname, 'popover.html'),
        manager: resolve(import.meta.dirname, 'manager.html'),
      },
    },
  },
});
