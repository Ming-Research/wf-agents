// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Chart } from './Chart';
import { matrix } from './matrix';
import { cell, flowOf } from './testing';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// A plan that research writes, design reads and writes, implement reads and writes, and review reads: the row's
/// line runs from research to review, and a way back from implement to design along a stretch of it.
function drawn(): HTMLElement {
  const flow = flowOf(
    [
      cell('research', { outputs: ['plan'] }),
      cell('design', { inputs: ['plan'], outputs: ['plan'] }),
      cell('implement', { inputs: ['plan'], outputs: ['plan'] }),
      cell('review', { inputs: ['plan'] }),
    ],
    { plan: {} },
  );
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
  it('keeps to its lane beside the line, and neither covers the other', () => {
    const [research, design, implement, review] = crossings(drawn());
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
