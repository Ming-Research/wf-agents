// The limit the page's browser tests are given (`vite.config.ts`, the browser project): a minute for one test, as the
// Rust tests are given; vitest's own is 15 s, which a busy machine came within 1.5 s of. It is read from the test's own
// task, as vitest sets it, so that a limit that does not reach the browser project fails here and not in a run that
// is slow. A few milliseconds beyond the file's load; the file is the only one that tells this limit from the default.

import { describe, expect, it } from 'vitest';

describe('a test of the page in the browser', () => {
  it('is given a minute', ({ task }) => {
    expect((task as { timeout?: number }).timeout).toBe(60_000);
  });
});
