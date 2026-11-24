// Whether the page shows made-up data (the fixtures) in place of the engine's. Only the dev server and the tests ever
// do: the page built into the program shows the engine's data alone, and holds no made-up data, which the build leaves
// out as code that never runs. On the dev server it does when the address has `?fixtures`; and, while the page is
// being designed, when the dev server was started with `VITE_FIXTURES=1`, unless the address has `?live`, which shows
// the engine's data.

export function usesFixtures(search: string): boolean {
  return import.meta.env.DEV && fixturesAsked(search);
}

function fixturesAsked(search: string): boolean {
  const asked = new URLSearchParams(search);
  if (asked.has('fixtures')) return true;
  const env = (import.meta as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_FIXTURES === '1' && !asked.has('live');
}

/// How many projects the fixtures name where the address asks for a number of them, as `?fixtures&projects=12`; none
/// where it asks for none, or for no whole number above nought, and the fixtures name the projects they name alone.
export function projectsAsked(search: string): number | null {
  const asked = new URLSearchParams(search).get('projects');
  if (asked === null || !/^\d+$/.test(asked)) return null;
  const n = Number(asked);
  return n > 0 ? n : null;
}

// Whether the page fills what the engine does not give yet with made-up content (`console/mock.ts`, `engine-mock.ts`),
// so that the whole designed page can be looked at; as the fixtures, on the dev server and in the tests only: when the
// dev server was started with `VITE_MOCKS=1`, or the address has `?mocks`. Off, the page shows only what the engine
// gives, and draws nothing where it gives nothing.

export function usesMocks(search: string): boolean {
  return import.meta.env.DEV && mocksAsked(search);
}

function mocksAsked(search: string): boolean {
  if (new URLSearchParams(search).has('mocks')) return true;
  const env = (import.meta as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_MOCKS === '1';
}

/// Whether this page shows the mocks, by its address and how its server was started.
export const mocksShown = (): boolean => usesMocks(typeof window === 'undefined' ? '' : window.location.search);
