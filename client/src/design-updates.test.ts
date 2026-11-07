// The design as updated while implementing and fixing, on an item's page: which passages stand, the round each came
// from, and how they are drawn after the design.

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { JsonValue, Shown } from './api';
import { DesignUpdates, designUpdates } from './pages/Item';

const passage = (name: string): JsonValue => ({ passage: name, decision: `${name} decided`, why: `${name} because` });
const shown = (work: number, round: number, cell: string, value: JsonValue): Shown => ({
  work,
  round,
  cell,
  path: 'design_updates',
  value,
});

describe('the design as updated while implementing', () => {
  it("takes each cell's newest return of the latest work, each passage at the round it first came from, oldest first", () => {
    const list = [
      shown(1, 30, 'I1', [passage('an earlier work')]),
      shown(2, 5, 'I1', [passage('p1'), passage('taken back')]),
      shown(2, 9, 'I1', [passage('p1'), passage('p2')]),
      shown(2, 14, 'T6', [passage('p3')]),
    ];
    const got = designUpdates(list).map((u) => [u.passage, u.from.cell, u.from.round]);
    expect(got).toEqual([
      ['p1', 'I1', 5],
      ['p2', 'I1', 9],
      ['p3', 'T6', 14],
    ]);
  });

  it('takes a passage written again in other words as written in the round that changed it', () => {
    const list = [
      shown(2, 5, 'I1', [passage('p1')]),
      shown(2, 9, 'I1', [{ passage: 'p1', decision: 'p1 decided otherwise', why: 'p1 because' }]),
    ];
    const got = designUpdates(list);
    expect(got.map((u) => [u.decision, u.from.round])).toEqual([['p1 decided otherwise', 9]]);
  });

  it('takes the passages back when the newest return of the cell holds none', () => {
    expect(designUpdates([shown(2, 5, 'I1', [passage('p1')]), shown(2, 9, 'I1', [])])).toEqual([]);
    expect(designUpdates(undefined)).toEqual([]);
  });

  it('draws each passage under its heading with its decision and why, in order', () => {
    const html = renderToStaticMarkup(
      createElement(DesignUpdates, {
        updates: designUpdates([shown(2, 5, 'I1', [passage('p1')]), shown(2, 14, 'T6', [passage('p3')])]),
      }),
    );
    expect(html).toContain('Updated while implementing');
    const at = (text: string) => html.indexOf(text);
    expect(at('p1 decided')).toBeGreaterThan(-1);
    expect(at('p1 because')).toBeGreaterThan(at('p1 decided'));
    expect(at('p3 decided')).toBeGreaterThan(at('p1 because'));
  });
});
