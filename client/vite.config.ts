// Builds the page into dist/, which crates/web builds into the wf-agents binary. `npm run dev` serves the page with
// Vite and passes the interface's requests to a running `wf-agents flow page` (WF_AGENTS_URL, by default the port it
// takes); the engine refuses a request of another origin, so they are passed on as from its own, the Host and the
// Origin both the engine's. The set-up is brought over from the page of the engine on `old-main`. `npm run dev` also
// serves `console.html`, the console alone, which is not built.
//
// The page's tests are two Vitest projects, so that each file runs where it needs to and the browser starts only for
// the files that need it:
//   unit     - the tests in Node, and in happy-dom for a file that says so (`// @vitest-environment happy-dom`);
//   browser  - the tests in `*.browser.test.tsx`, which draw the page in a real Chromium: WebGL, layout, the pointer and
//              the keyboard are the browser's own, which happy-dom does not have.
// Both brought over from the page on `old-main`, which had a third, `ui`, for a check of the page against its UI spec,
// which this page has no such spec for yet.
import { playwright } from '@vitest/browser-playwright';
import react from '@vitejs/plugin-react';
import { configDefaults, defineConfig } from 'vitest/config';

const target = process.env.WF_AGENTS_URL ?? 'http://127.0.0.1:7490';

const BROWSER_TESTS = 'src/**/*.browser.test.tsx';

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  test: {
    // The page's own CSS is read as written, so that a test drawn in the browser is laid out as the page is; Vitest
    // otherwise gives every CSS file as empty.
    css: { include: [/\/src\/.+\.css/] },
    projects: [
      {
        extends: true,
        test: { name: 'unit', exclude: [...configDefaults.exclude, BROWSER_TESTS] },
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
      '/api': { target, changeOrigin: true, headers: { origin: new URL(target).origin } },
    },
  },
});
