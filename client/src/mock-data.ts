// Whether the page shows made-up data (the fixtures) in place of the engine's. It does when the address has
// `?fixtures`; and, while the page is being designed, when the dev server was started with `VITE_FIXTURES=1`, unless
// the address has `?live`, which shows the engine's data.

export function usesFixtures(search: string): boolean {
  const asked = new URLSearchParams(search);
  if (asked.has('fixtures')) return true;
  const env = (import.meta as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_FIXTURES === '1' && !asked.has('live');
}

// Whether the page fills what the engine does not give yet with made-up content (`console/mock.ts`, `engine-mock.ts`),
// so that the whole designed page can be looked at: when the dev server was started with `VITE_MOCKS=1`, or the address
// has `?mocks`. Off, the page shows only what the engine gives, and draws nothing where it gives nothing.

export function usesMocks(search: string): boolean {
  if (new URLSearchParams(search).has('mocks')) return true;
  const env = (import.meta as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_MOCKS === '1';
}

/// Whether this page shows the mocks, by its address and how its server was started.
export const mocksShown = (): boolean => usesMocks(typeof window === 'undefined' ? '' : window.location.search);
