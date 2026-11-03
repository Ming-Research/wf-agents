// A probe of the fast CI (#269): a page test that fails. Reverted in the next commit.
import { describe, expect, it } from 'vitest';

describe('the probe', () => {
  it('fails on purpose in the probe', () => {
    expect(1).toBe(2);
  });
});
