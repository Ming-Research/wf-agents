// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Engine } from './Engine';
import type { Engine as EngineState } from './works-api';
import { fixtureItems } from './works/fixtures';
import { type Source, SourceContext } from './works/source';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
});

/// The bar's engine part, on an engine that stands so, and the fixtures' items.
async function drawn(engine: EngineState): Promise<HTMLElement> {
  const source = {
    loadEngine: async () => engine,
    changes: () => () => undefined,
  } as unknown as Source;
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SourceContext.Provider value={source}>
        <Engine items={fixtureItems()} failed={null} />
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return container;
}

describe("the bar's engine part", () => {
  // The stop is asked for on the command line alone: the bar only says that it stands, where the engine's state is
  // said, drawn as a pause the rounds finish under, and offers nothing to stop with.
  it('says the engine is stopping while a stop stands, with the rounds it waits for, and offers no stop', async () => {
    const running = fixtureItems().items.filter((i) => i.work.now.kind === 'round').length;
    expect(running).toBeGreaterThan(0);
    const stopping = await drawn({
      slots: 3,
      taken: 1,
      paused: null,
      keepWarm: true,
      stopping: { since: Date.now(), atOnce: false },
    });
    const words = stopping.querySelector('.engine-words')?.textContent;
    expect(words).toBe(`Stopping: ${running} ${running === 1 ? 'round' : 'rounds'} finishing`);
    expect(stopping.querySelector('.engine-dot-pausing')).not.toBeNull();
    expect(stopping.querySelector('.engine-state')?.getAttribute('title')).toContain('asked to stop');
    const controls = [...stopping.querySelectorAll('button')].map((b) => b.textContent ?? '');
    expect(controls).toContain('Pause all');
    expect(controls.filter((c) => /stop/i.test(c))).toEqual([]);

    const going = await drawn({ slots: 3, taken: 1, paused: null, keepWarm: true });
    expect(going.querySelector('.engine-words')?.textContent).toMatch(/^Running/);
  });
});
