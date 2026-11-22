// The limit the page's unit tests are given: vitest's own five seconds, which the browser project's minute
// (`vite.config.ts`) must not reach. Read from the test's own task, as vitest sets it. 8 ms for the file in the page job of run 45005452666: it
// is the only test that says the browser project's limit stayed in the browser project.

import { describe, expect, it } from 'vitest';

describe('a test of the page in Node', () => {
  it('is given five seconds', ({ task }) => {
    expect((task as { timeout?: number }).timeout).toBe(5_000);
  });
});
