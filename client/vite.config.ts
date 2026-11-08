// Builds the page into dist/, which crates/web builds into the wf-agents binary. `npm run dev` serves the page with
// Vite and passes the interface's requests to a running `wf-agents run` (WF_AGENTS_URL, by default the port run
// takes); the interface refuses requests of another origin, so they are passed on as from its own.
//
// The page's tests are three Vitest projects, so that each file runs where it needs to and the browser starts only for
// the files that need it:
//   unit     - the tests in Node, and in happy-dom for a file that says so (`// @vitest-environment happy-dom`);
//   ui       - the check of the page against its design system (design/wf-agents/frontend/design-system.md), which CI
//              runs as its own step (`npm run check:ui`);
//   browser  - the tests in `*.browser.test.tsx`, which draw the page in a real Chromium: layout, the pointer and the
//              keyboard are the browser's own, which happy-dom does not have.
import { playwright } from '@vitest/browser-playwright';
import react from '@vitejs/plugin-react';
import { configDefaults, defineConfig } from 'vitest/config';

const target = process.env.WF_AGENTS_URL ?? 'http://127.0.0.1:7480';

const BROWSER_TESTS = 'src/**/*.browser.test.tsx';
const UI_CHECK = 'src/ui-check.test.ts';

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    // The graphs' layout engine is large; the page is served from the same machine.
    chunkSizeWarningLimit: 4096,
  },
  test: {
    // Primer's components import their own CSS, which Node cannot load: the tests take them through Vite, which leaves
    // the CSS out.
    server: { deps: { inline: [/@primer\/react/] } },
    // The page's own CSS is read as written, for the UI check; Vitest otherwise gives every CSS file as empty.
    css: { include: [/\/src\/.+\.css/] },
    projects: [
      {
        extends: true,
        test: { name: 'unit', exclude: [...configDefaults.exclude, BROWSER_TESTS, UI_CHECK] },
      },
      {
        extends: true,
        test: { name: 'ui', include: [UI_CHECK] },
      },
      {
        extends: true,
        test: {
          name: 'browser',
          include: [BROWSER_TESTS],
          browser: {
            enabled: true,
            headless: true,
            provider: playwright(),
            // The window of a desktop; the page lays out for one.
            viewport: { width: 1280, height: 800 },
            // A failure is told in the test's log; the screenshot would be a file in the repository's tree.
            screenshotFailures: false,
            instances: [{ browser: 'chromium' }],
          },
        },
      },
    ],
  },
  server: {
    proxy: {
      '/api': { target, changeOrigin: true, headers: { origin: target } },
    },
  },
});
