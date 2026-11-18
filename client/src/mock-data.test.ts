// The page shows made-up data only where the dev server or the tests turn it on: the page built into the program, whose
// `import.meta.env.DEV` the build sets false, never does, whatever its address says. Neither the tests nor the dev
// server here are started with the switches that turn it on for every address.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { usesFixtures, usesMocks } from './mock-data';

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('the made-up data', () => {
  it('is shown on the dev server where the address asks for it, and not where it asks for the engine’s', () => {
    vi.stubEnv('DEV', true);
    expect(usesFixtures('?fixtures')).toBe(true);
    expect(usesFixtures('?live')).toBe(false);
    expect(usesMocks('?mocks')).toBe(true);
    expect(usesMocks('?fixtures')).toBe(false);
  });

  it('is never shown by the page built into the program, whatever its address asks for', () => {
    vi.stubEnv('DEV', false);
    expect(usesFixtures('?fixtures')).toBe(false);
    expect(usesFixtures('')).toBe(false);
    expect(usesMocks('?mocks')).toBe(false);
    expect(usesMocks('')).toBe(false);
  });
});
