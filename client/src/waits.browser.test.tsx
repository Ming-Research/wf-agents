// How long the page's browser tests wait on a condition (`vite.config.ts`, the browser project): longer than the second
// vitest gives a poll by default, which a busy machine crossed. About 1.3 s alone: the wait it must outlast is one
// second, so it cannot be shorter, and it fails only where the setting does not reach the browser project, which no
// other test shows except by failing now and then on a busy machine.

import { describe, expect, it } from 'vitest';

describe('a wait on a condition', () => {
  it("holds past the second vitest's poll is given by default", async () => {
    let held = false;
    setTimeout(() => {
      held = true;
    }, 1300);
    await expect.poll(() => held).toBe(true);
  });
});
