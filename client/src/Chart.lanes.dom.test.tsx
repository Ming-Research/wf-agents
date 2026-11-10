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

/// A plan that research writes, design reads and writes, implement reads and writes, and review reads: the row's
/// line runs from research to review, and a way back from implement to design along a stretch of it.
const BOTH = () =>
  flowOf(
    [
      cell('research', { outputs: ['plan'] }),
      cell('design', { inputs: ['plan'], outputs: ['plan'] }),
      cell('implement', { inputs: ['plan'], outputs: ['plan'] }),
      cell('review', { inputs: ['plan'] }),
    ],
    { plan: {} },
  );

/// Opinions that two cells read and a cell after them writes: nothing of them goes forward.
const BACK_ALONE = () =>
  flowOf([cell('plan', { inputs: ['opinions'] }), cell('build', { inputs: ['opinions'] }), cell('judge', { outputs: ['opinions'] })], {
    opinions: {},
  });

function drawn(flow: Flow): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<Chart matrix={matrix({ after: flow }, 'after')} chosen={null} onChoose={() => undefined} />));
  unmount = () => act(() => root.unmount());
  return container;
}

/// What each crossing of the one row draws: its segments and its arrow, by their classes.
const crossings = (page: HTMLElement) =>
  [...page.querySelectorAll('tbody tr td.spot')].map((spot) =>
    [...spot.querySelectorAll('.seg, .arrow')].map((part) => part.className.trim().replace(/\s+/g, ' ')),
  );

describe('a way back along the row\'s line', () => {
  it('runs alone in the middle where the row has no line, with its arrow at each read it reaches', () => {
    const [plan, build, judge] = crossings(drawn(BACK_ALONE()));
    expect(plan).toEqual(['seg seg-right back', 'arrow']);
    // The way back passes through this read to the one before it.
    expect(build).toEqual(['seg seg-left back', 'seg seg-right back', 'arrow']);
    expect(judge).toEqual(['seg seg-left back']);
  });

  it('keeps to its lane beside the line, and neither covers the other', () => {
    const [research, design, implement, review] = crossings(drawn(BOTH()));
    // Before the way back the line runs alone, in the middle.
    expect(research).toEqual(['seg seg-right line']);
    // Between the read the work goes back to and the write that sends it, both run, each in its lane; the arrow is
    // in the way back's lane.
    expect(design).toEqual(['seg seg-left line', 'seg seg-right line lane', 'seg seg-right back lane', 'arrow lane']);
    expect(implement).toEqual(['seg seg-left line lane', 'seg seg-left back lane', 'seg seg-right line']);
    // After it the line runs alone again.
    expect(review).toEqual(['seg seg-left line']);
  });
});
