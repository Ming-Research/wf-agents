// Whether the page shows made-up data (the fixtures) in place of the engine's. It does when the address has
// `?fixtures`; and, while the page is being designed, when the dev server was started with `VITE_FIXTURES=1`, unless
// the address has `?live`, which shows the engine's data.

export function usesFixtures(search: string): boolean {
  const asked = new URLSearchParams(search);
  if (asked.has('fixtures')) return true;
  const env = (import.meta as { env?: Record<string, string | undefined> }).env;
  return env?.VITE_FIXTURES === '1' && !asked.has('live');
}
