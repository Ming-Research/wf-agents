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

/// The bar's part of the engine over these items, the engine standing so: only what the part reads of the engine is
/// given.
async function bar(engine: EngineState): Promise<HTMLElement> {
  const source = {
    loadEngine: async () => engine,
    actOnEngine: async () => null,
    changes: () => () => {},
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

describe("the engine's part of the bar", () => {
  // Under the pause of everything a round still begins for what the owner said or answered, as under a work's own
  // pause: the switch says so before it is pressed, as the command line says it.
  it("says before pausing everything that no round begins but for the owner's words or answers", async () => {
    const container = await bar({ slots: 2, taken: 0, paused: null });
    const pause = [...container.querySelectorAll('button')].find((b) => b.textContent === 'Pause all');
    expect(pause?.getAttribute('title')).toBe(
      "No work begins a round but for the owner's words or answers, until every work is started again; a round under way finishes.",
    );
  });
});
