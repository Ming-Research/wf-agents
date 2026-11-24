// The page shows made-up data only where the dev server or the tests turn it on: the page built into the program, whose
// `import.meta.env.DEV` the build sets false, never does, whatever its address says. Neither the tests nor the dev
// server here are started with the switches that turn it on for every address.

import { afterEach, describe, expect, it, vi } from 'vitest';

import { projectsAsked, usesFixtures, usesMocks } from './mock-data';

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

  it('names as many projects as the address asks for, and the fixtures’ own where it asks for no count above nought', () => {
    expect(projectsAsked('?fixtures&projects=12')).toBe(12);
    expect(projectsAsked('?projects=1&fixtures')).toBe(1);
    expect(projectsAsked('?fixtures')).toBeNull();
    expect(projectsAsked('?fixtures&projects=0')).toBeNull();
    expect(projectsAsked('?fixtures&projects=-2')).toBeNull();
    expect(projectsAsked('?fixtures&projects=two')).toBeNull();
    expect(projectsAsked('?fixtures&projects=')).toBeNull();
  });

  it('is never shown by the page built into the program, whatever its address asks for', () => {
    vi.stubEnv('DEV', false);
    expect(usesFixtures('?fixtures')).toBe(false);
    expect(usesFixtures('')).toBe(false);
    expect(usesMocks('?mocks')).toBe(false);
    expect(usesMocks('')).toBe(false);
  });
});
