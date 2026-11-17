// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Chart } from './Chart';
import { matrix } from './matrix';
import type { Flow } from './api';
import { cell, flowOf } from './testing';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// A plan that research writes, design and implement each watch and write, and review watches: the way back from
/// implement to design is the only line along its row.
const BOTH = () =>
  flowOf(
    [
      cell('research', { outputs: ['plan'] }),
      cell('design', { watches: ['plan'], outputs: ['plan'] }),
      cell('implement', { watches: ['plan'], outputs: ['plan'] }),
      cell('review', { watches: ['plan'] }),
    ],
    { plan: {} },
  );

/// Opinions that two cells watch and a cell after them writes.
const BACK_ALONE = () =>
  flowOf(
    [
      cell('plan', { watches: ['opinions'] }),
      cell('build', { watches: ['opinions'] }),
      cell('judge', { outputs: ['opinions'] }),
    ],
    { opinions: {} },
  );

function drawn(flow: Flow): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<Chart matrix={matrix(flow)} chosen={null} onChoose={() => undefined} />));
  unmount = () => act(() => root.unmount());
  return container;
}

/// What each crossing of a datum's row draws: its segments and its arrow, by their classes.
const crossings = (page: HTMLElement, block: string) =>
  [...(page.querySelector(`th[data-key="block:${block}"]`)?.closest('tr')?.querySelectorAll('td.spot') ?? [])].map(
    (spot) =>
      [...spot.querySelectorAll('.seg, .arrow')].map((part) =>
        (part.getAttribute('class') ?? '').trim().replace(/\s+/g, ' '),
      ),
  );

describe('a way back along a datum\'s row', () => {
  it('runs in the middle of the row, with its arrow at each cell that watches it and it runs again', () => {
    const [plan, build, judge] = crossings(drawn(BACK_ALONE()), 'opinions');
    expect(plan).toEqual(['seg seg-right back', 'part arrow']);
    // The way back passes through this watcher to the one before it.
    expect(build).toEqual(['seg seg-left back', 'seg seg-right back', 'part arrow']);
    expect(judge).toEqual(['seg seg-left back']);
  });

  it('is the only line along the row: a write going right draws none', () => {
    const [research, design, implement, review] = crossings(drawn(BOTH()), 'plan');
    expect(research).toEqual([]);
    expect(design).toEqual(['seg seg-right back', 'part arrow']);
    expect(implement).toEqual(['seg seg-left back']);
    expect(review).toEqual([]);
  });
});
