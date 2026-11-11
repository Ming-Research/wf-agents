// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Copies, CopiesState } from '../works-api';
import { copiesWords } from './Copies';
import { fixtureCopies, fixtureItems } from './fixtures';
import { type Source, SourceContext } from './source';
import { WorksTable } from './WorksList';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

const HOUR = 60 * 60 * 1000;

/// The copies as the engine gives them in this state, at the fixtures' time.
function copiesIn(state: CopiesState, over: Partial<Copies> = {}): Copies {
  return { ...fixtureCopies(), state, ...over };
}

/// A source whose copies are read by `copies`, which fails where it throws; the rest is not read by the line.
function sourceOf(copies: () => Promise<Copies>): Source {
  const never = () => Promise.reject(new Error('not read'));
  return {
    loadCopies: copies,
    loadItems: async () => fixtureItems(),
    loadWork: never,
    loadVersion: never,
    loadRound: never,
    loadTold: never,
    loadCall: never,
    act: async () => null,
    changes: () => () => undefined,
  };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/// Draws the works table, whose line holds the copies, and gives the line.
async function line(copies: () => Promise<Copies>): Promise<HTMLElement | null> {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SourceContext.Provider value={sourceOf(copies)}>
        <WorksTable items={fixtureItems()} />
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  await settle();
  return container.querySelector<HTMLElement>('.view-line .copies');
}

describe('the line of the copies of the database', () => {
  it('is plain, and not an alert, where the newest copy is recent and kept off the machine', async () => {
    const shown = await line(async () => copiesIn({ kind: 'ok' }, { lastTaken: fixtureCopies().now - 3 * HOUR }));
    expect(shown?.textContent).toBe('The newest copy of the database was taken 3h 0m ago, and is kept off this machine');
    expect(shown?.className).toBe('copies quiet');
    expect(shown?.getAttribute('role')).toBeNull();
  });

  const warned: [string, CopiesState, 'warn' | 'failed', string][] = [
    ['kept on this machine only', { kind: 'localOnly' }, 'warn', 'stay on this machine, on the disk they guard against losing'],
    ['not yet kept off the machine', { kind: 'pending', n: 2 }, 'warn', '2 copies of the database not yet kept off this machine'],
    ['too old', { kind: 'stale', since: fixtureCopies().now - 30 * HOUR }, 'warn', 'was taken 1d 6h ago'],
    ['none taken', { kind: 'stale', since: null }, 'warn', 'No copy of the database has been taken'],
    ['dated in the future', { kind: 'clock' }, 'warn', "the engine's clock is wrong"],
    [
      'failed',
      { kind: 'failed', at: fixtureCopies().now - HOUR, step: 'upload', why: 'the place cannot be reached' },
      'failed',
      "Keeping the database's copies failed at upload: the place cannot be reached",
    ],
  ];
  for (const [name, state, tone, words] of warned) {
    it(`warns, with its reason, where the copies are ${name}`, async () => {
      const shown = await line(async () => copiesIn(state));
      expect(shown?.textContent).toContain(words);
      expect(shown?.className).toBe(`copies ${tone}`);
      expect(shown?.getAttribute('role')).toBe('alert');
      expect(shown?.getAttribute('data-state')).toBe(state.kind);
    });
  }

  it('says that the copies could not be read, as a failure, rather than showing nothing', async () => {
    const shown = await line(() => Promise.reject(new Error('the engine answered 503')));
    expect(shown?.textContent).toBe('The copies of the database could not be read: the engine answered 503');
    expect(shown?.className).toBe('copies failed');
  });

  it('says one copy in the singular', () => {
    expect(copiesWords(copiesIn({ kind: 'pending', n: 1 }))).toBe('1 copy of the database not yet kept off this machine');
  });
});
