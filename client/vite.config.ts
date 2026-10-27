// Builds the page into dist/, which crates/web builds into the wf-agents binary. `npm run dev` serves the page with
// Vite and passes the interface's requests to a running `wf-agents run` (WF_AGENTS_URL, by default the port run
// takes); the interface refuses requests of another origin, so they are passed on as from its own.
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

const target = process.env.WF_AGENTS_URL ?? 'http://127.0.0.1:7480';

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // The graphs' layout engine is large; the page is served from the same machine.
    chunkSizeWarningLimit: 4096,
  },
  server: {
    proxy: {
      '/api': { target, changeOrigin: true, headers: { origin: target } },
    },
  },
});
