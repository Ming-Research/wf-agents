// Builds the page into dist/, which crates/web builds into the wf-agents binary. `npm run dev` serves the page with
// Vite and passes the interface's requests to a running `wf-agents flow page` (WF_AGENTS_URL, by default the port it
// takes). The set-up is brought over from the page of the engine on `main`.
//
// The page's tests run in Node, and in happy-dom for a file that says so (`// @vitest-environment happy-dom`), which
// draws the page in a DOM without a browser to start.
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

const target = process.env.WF_AGENTS_URL ?? 'http://127.0.0.1:7490';

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/api': { target, changeOrigin: true },
    },
  },
});
