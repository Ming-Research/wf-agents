// The section of a comparison's panel that tells what installing the flow shown would do with each work going on: the
// works in runs by what happens to them in the order the engine gives, the runs that cost something open and the others
// folded, each work with where it stands and what happens to it in the engine's words, and, where the engine cannot
// judge the flow, the reason in place of the runs.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { FlowCost, WorkCost, WorkStands } from './api';
import { CostSection, costRuns } from './flow/Cost';
import { costPath } from './pages/Flow';
import { t } from './text';

const stands = (over: Partial<WorkStands> = {}): WorkStands => ({
  item: 1,
  title: 'Back',
  project: null,
  graph: 'engineer',
  version: 1,
  at: 'Z1',
  paused: null,
  ...over,
});

/// One work of every kind, as the engine lists them: by kind, then by work.
const WORKS: WorkCost[] = [
  {
    work: 8,
    stands: stands({ item: 5, title: 'Broken', version: 3, at: 'R1' }),
    effect: { kind: 'unreadable', why: 'flow version 3: does not read' },
  },
  {
    work: 1,
    stands: stands(),
    effect: { kind: 'goes_back', through: 'Y1', to: 'R1', why: 'cell Z1 is gone', from: 1, redone: 1 },
  },
  {
    work: 9,
    stands: stands({ item: 6, title: 'Later', at: 'R3' }),
    effect: { kind: 'goes_back', through: 'Y1', to: 'R2', why: 'the return no longer fits', from: 4, redone: 3 },
  },
  {
    work: 2,
    stands: stands({ item: 2, title: 'Stuck', paused: 'owner' }),
    effect: { kind: 'stuck', at: 'Y1', pause: 'Start it over, or cancel its item' },
  },
  {
    work: 4,
    stands: stands({ item: null, title: null, project: 'p', graph: 'policy', at: 'P1' }),
    effect: { kind: 'ends', why: 'the version has no graph policy', taken_over: false },
  },
  { work: 3, stands: stands({ item: 3, title: 'Moves', at: 'R0' }), effect: { kind: 'moves' } },
  {
    work: 5,
    stands: stands({ item: null, title: null, project: 'p', graph: 'architect', version: 3, at: 'N0' }),
    effect: { kind: 'stays' },
  },
];

const COST: FlowCost = { version: 3, stored: false, works: WORKS };

const TITLES: Record<string, string> = { 'engineer:R1': 'Whether it stands', 'engineer:Y1': 'Version check' };
const titleOf = (graph: string, cell: string) => TITLES[`${graph}:${cell}`];

const drawn = (cost: FlowCost) =>
  renderToStaticMarkup(<CostSection cost={{ data: cost, error: null, stale: false }} titleOf={titleOf} />);

/// What a reader sees of markup: its text.
const text = (html: string) => html.replace(/<[^>]*>/g, '');

describe('the cost of installing a flow, on the comparison panel', () => {
  it('lists the works in runs by what happens to them, in the engine order, the ones that cost something open', () => {
    const html = drawn(COST);
    const said = text(html);
    const heads = [
      '1 cannot be worked out, and the engine stops when it brings it up',
      '2 go back and do work again',
      '1 stays stuck, paused',
      '1 ends',
      '1 moves where it stands',
      '1 already on this version',
    ];
    const at = heads.map((h) => said.indexOf(h));
    expect(at.every((i) => i >= 0), said).toBe(true);
    expect([...at].sort((a, b) => a - b)).toEqual(at);
    // Four runs cost something and are open; the two that go on as they are are folded.
    expect(html.match(/<details/g)).toHaveLength(6);
    expect(html.match(/<details[^>]* open=""/g)).toHaveLength(4);
    expect(said).toContain('Installing it makes it flow version 3.');
    expect(said).toContain('Works going on');
  });

  it('says of each work where it stands and what happens to it, in the cells of the flow shown', () => {
    const said = text(drawn(COST));
    expect(said).toContain('W1');
    expect(said).toContain('#1 Back');
    expect(said).toContain('on v1 at Z1');
    expect(said).toContain('through Y1 Version check back to R1 Whether it stands, doing round R1-1 again: cell Z1 is gone');
    expect(said).toContain('through Y1 Version check back to R2, doing round R9-4 and the 2 after it again: the return no longer fits');
    expect(said).toContain('stays at Y1 Version check, paused: Start it over, or cancel its item');
    expect(said).toContain('paused by the owner');
    expect(said).toContain('the version has no graph policy');
    expect(said).toContain('flow version 3: does not read');
    // A work with no item is named by the graph it runs through and its project.
    expect(said).toContain('policy work for p');
    expect(said).toContain('architect work for p');
  });

  it('names the cell a work stands at by its code alone, since the flow shown may give that code another meaning', () => {
    const said = text(drawn(COST));
    // Work 8 stands at R1 of its own version 3; the flow shown has an R1, with a title that is not that cell's.
    expect(TITLES['engineer:R1']).toBeDefined();
    expect(said).toContain('on v3 at R1');
    expect(said).not.toContain('on v3 at R1 Whether it stands');
    // The cells the flow shown is judged to go through and to keep their titles from it.
    expect(said).toContain('back to R1 Whether it stands');
  });

  it('gives a line of what happens only to the works it happens to something', () => {
    const html = drawn(COST);
    // The works that cannot be worked out, that go back, are stuck or end: five. Those that go on as they are have none.
    expect(html.match(/class="cost-effect"/g)).toHaveLength(5);
  });

  it('says a flow already stored is, and what a flow with no work going on holds', () => {
    expect(text(drawn({ version: 2, stored: true, works: [] }))).toContain(
      'This flow is flow version 2, which the database has.',
    );
    expect(text(drawn({ version: 2, stored: true, works: [] }))).toContain('No work is going on.');
  });

  it('tells why the flow cannot be judged here, in place of the works', () => {
    const html = renderToStaticMarkup(
      <CostSection cost={{ data: undefined, error: 'flow/flow.json does not compile', stale: false }} titleOf={titleOf} />,
    );
    expect(text(html)).toContain('The works going on cannot be judged against this flow here: flow/flow.json does not compile');
    expect(text(html)).toContain('wf-agents flow cost');
    expect(html).not.toContain('<details');
    const waiting = renderToStaticMarkup(
      <CostSection cost={{ data: undefined, error: null, stale: false }} titleOf={titleOf} />,
    );
    expect(text(waiting)).toContain(t('loading'));
  });

  it('puts the works in runs of one kind as they are given, and no kind twice unless it is given twice', () => {
    expect(costRuns(WORKS).map((r) => [r.kind, r.works.map((w) => w.work)])).toEqual([
      ['unreadable', [8]],
      ['goes_back', [1, 9]],
      ['stuck', [2]],
      ['ends', [4]],
      ['moves', [3]],
      ['stays', [5]],
    ]);
    expect(costRuns([])).toEqual([]);
  });
});

describe('which flows the cost is read for', () => {
  const route = { page: 'flow' as const, version: null, graph: null, cell: null, project: null, ref: null, snapshot: null, from: null };

  it('is the flow at a git ref that is compared, and no other: not a stored version, a decision card or a ref alone', () => {
    expect(costPath({ ...route, project: 'p', ref: 'wf-agents/1', from: 3 })).toBe(
      '/api/v1/flows/cost?project=p&ref=wf-agents%2F1',
    );
    // A stored version is installed already, and a decision card's flow is no branch to install.
    expect(costPath({ ...route, version: 2, from: 1 })).toBeNull();
    expect(costPath({ ...route, snapshot: 4, from: 1 })).toBeNull();
    // A ref shown with no comparison is not being reviewed.
    expect(costPath({ ...route, project: 'p', ref: 'main' })).toBeNull();
  });
});
