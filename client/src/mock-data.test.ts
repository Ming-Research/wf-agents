// The page shows made-up data only where the dev server or the tests turn it on: the page built into the program, whose
// `import.meta.env.DEV` the build sets false, never does, whatever its address says.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { usesFixtures, usesMocks } from './mock-data';

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('the made-up data', () => {
  it('is shown in place of the engine’s on the dev server where the address asks for it or the server was started so, unless the address asks for the engine’s', () => {
    vi.stubEnv('DEV', true);
    vi.stubEnv('VITE_FIXTURES', '');
    expect(usesFixtures('')).toBe(false);
    expect(usesFixtures('?fixtures')).toBe(true);
    vi.stubEnv('VITE_FIXTURES', '1');
    expect(usesFixtures('')).toBe(true);
    expect(usesFixtures('?live')).toBe(false);
  });

  it('fills what the engine does not give on the dev server only where the address or the server asks for it', () => {
    vi.stubEnv('DEV', true);
    vi.stubEnv('VITE_MOCKS', '');
    expect(usesMocks('')).toBe(false);
    expect(usesMocks('?mocks')).toBe(true);
    vi.stubEnv('VITE_MOCKS', '1');
    expect(usesMocks('')).toBe(true);
  });

  it('is never shown by the page built into the program, whatever its address or its build asked for', () => {
    vi.stubEnv('DEV', false);
    vi.stubEnv('VITE_FIXTURES', '1');
    vi.stubEnv('VITE_MOCKS', '1');
    expect(usesFixtures('?fixtures')).toBe(false);
    expect(usesFixtures('')).toBe(false);
    expect(usesMocks('?mocks')).toBe(false);
    expect(usesMocks('')).toBe(false);
  });
});
