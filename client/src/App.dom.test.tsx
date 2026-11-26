// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { App, Shell } from './App';
import { fixtureItems } from './works/fixtures';
import { INTERFACE, SourceContext } from './works/source';
import type { InstallationWait } from './works-api';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  window.location.hash = '';
  vi.unstubAllGlobals();
});

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe('the page on a server that gives the flow alone', () => {
  it('reads nothing of the engine and draws no part of it in the bar', async () => {
    // `wf-agents flow page` answers that it has no such path for everything but the flow.
    const asked: string[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn((path: string, init?: RequestInit) => {
        asked.push(`${init?.method ?? 'GET'} ${path}`);
        return Promise.resolve(new Response('', { status: 404 }));
      }),
    );
    const streams: string[] = [];
    vi.stubGlobal(
      'EventSource',
      class {
        onopen: (() => void) | null = null;
        onmessage: (() => void) | null = null;
        constructor(url: string) {
          streams.push(url);
        }
        close() {
          // Nothing was opened.
        }
      },
    );
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    act(() => root.render(<App />));
    unmount = () => act(() => root.unmount());
    for (let i = 0; i < 3; i += 1) await settle();
    // The server was asked what it gives, and found to give the flow alone.
    expect(asked).toContain('HEAD /api/v1/levels');
    expect(window.location.hash).toBe('#/flow');
    expect(container.querySelector('.bar')).not.toBeNull();
    expect(container.querySelector('.engine')).toBeNull();
    expect(asked.filter((a) => a.startsWith('GET /api/v1/items') || a.startsWith('GET /api/v1/engine'))).toEqual([]);
    expect(streams).toEqual([]);
  });
});

describe('the red count beside Console', () => {
  const stopped: InstallationWait = {
    key: 'everything',
    why: 'The engine paused every work as it started again.',
    ends: { kind: 'resume' },
    since: 5000,
  };

  /// The red count the shell draws beside Console for the items of the fixtures, with these waits of the installation
  /// in the read of the items (none where the engine gives none).
  async function redCount(waits: InstallationWait[] | undefined): Promise<number> {
    const source = {
      ...INTERFACE,
      loadItems: async () => ({ ...fixtureItems(), waits }),
      loadEngine: undefined,
      changes: () => () => undefined,
    };
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    act(() =>
      root.render(
        <SourceContext.Provider value={source}>
          <Shell route={{ view: 'console' }}>
            <div />
          </Shell>
        </SourceContext.Provider>,
      ),
    );
    for (let i = 0; i < 3; i += 1) await settle();
    const text = container.querySelector('.bar-count:not(.ahead):not(.unread)')?.textContent;
    act(() => root.unmount());
    container.remove();
    return Number(text ?? 0);
  }

  it("counts the engine's pause of every work once, beside what the works wait on the owner for, and nothing where the installation waits for none", async () => {
    const works = await redCount([]);
    expect(await redCount(undefined)).toBe(works);
    expect(await redCount([stopped])).toBe(works + 1);
  });
});
