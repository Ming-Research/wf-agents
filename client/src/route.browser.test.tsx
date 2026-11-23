// The way the page replaces its own address, in a real browser (Chromium), whose hash changes and history are its own:
// an address the page replaces (`replaceHash`) is not one it brought the owner to, so going back from a view opened
// directly leads to the view's own way back and does not leave the page, and one it did bring the owner to is. Whether
// the page brought the owner is kept for as long as the page lives, so each test draws the page anew, from modules read
// again.

import { afterEach, describe, expect, it, vi } from 'vitest';

let unmount: (() => void) | undefined;
const was = window.location.hash;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  vi.restoreAllMocks();
  history.replaceState(null, '', was === '' ? window.location.pathname : was);
});

/// The page's address as it follows it, drawn from modules read anew, so that nothing is remembered of an earlier test.
async function page() {
  vi.resetModules();
  const [{ flushSync }, { createRoot }, route] = await Promise.all([
    import('react-dom'),
    import('react-dom/client'),
    import('./route'),
  ]);
  function Follow() {
    return <p id="route">{JSON.stringify(route.useRoute())}</p>;
  }
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  flushSync(() => root.render(<Follow />));
  unmount = () => flushSync(() => root.unmount());
  const back = vi.spyOn(window.history, 'back').mockImplementation(() => undefined);
  const shown = () => document.getElementById('route')?.textContent ?? '';
  return { route, back, shown };
}

describe('an address the page replaces', () => {
  it('is not one it brought the owner to: going back sets the way back and does not leave the page', async () => {
    const { route, back, shown } = await page();
    route.replaceHash('#/works/7/runs/2');
    await expect.poll(shown).toContain('"run":2');
    route.goBack('#/works');
    expect(back).not.toHaveBeenCalled();
    expect(window.location.hash).toBe('#/works');
  });

  it('is replaced to the address the page already has with nothing left behind: the next change is one it brought the owner to', async () => {
    // The page stands on an address with a hash, as a work's does: a hash that is empty is no fragment, and replacing
    // it would load the page again.
    history.replaceState(null, '', '#/works/3');
    const { route, back, shown } = await page();
    route.replaceHash('#/works/3');
    window.location.hash = '#/works/7';
    await expect.poll(shown).toContain('"work":7');
    route.goBack('#/works');
    expect(back).toHaveBeenCalledTimes(1);
  });
});
