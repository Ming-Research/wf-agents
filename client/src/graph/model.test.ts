import { describe, expect, it } from 'vitest';

import type { Cell, Flow } from '../api';
import { exclusive } from '../matrix';
import { cell, flowOf, test, tested } from '../testing';
// The engineer flow as `/api/v1/flow` gives it for `flow/engineer` (`wf-agents flow page flow/engineer`), a copy, so
// that what the chart draws of it is known; read again from the flow page to bring it up to date.
import engineerDefinition from './engineer-flow.json';
import {
  type Chart,
  type ChartEdge,
  cellNode,
  chartOf,
  contextOf,
  forkNode,
  litBy,
  outsideNode,
  shownArrows,
  wayInNode,
} from './model';

const engineer = (engineerDefinition as unknown as { flow: Flow }).flow;

/// Every arrow of a chart, those folded into the main path's steps too.
const everyArrow = (chart: Chart): ChartEdge[] => [
  ...chart.edges.filter((e) => e.kind === 'arrow'),
  ...chart.edges.flatMap((e) => e.folds),
];
/// The arrows of a chart, each as `writer>reader` with the names it carries; those folded into a step too.
const arrows = (chart: Chart) => Object.fromEntries(everyArrow(chart).map((e) => [`${e.from}>${e.to}`, e.label]));
/// The arrows drawn as lines of their own, each as `writer>reader`.
const drawn = (chart: Chart) => chart.edges.filter((e) => e.kind === 'arrow').map((e) => `${e.from}>${e.to}`);
const arrow = (chart: Chart, from: string, to: string) =>
  everyArrow(chart).find((e) => e.from === from && e.to === to);
const nodeOf = (chart: Chart, id: string) => chart.nodes.find((n) => n.id === id);
/// The steps of the main path, each as `source > target` by node, a branch out of a fork with its words after the
/// fork, in a sorted list.
const steps = (chart: Chart) =>
  chart.edges
    .filter((e) => e.kind === 'step')
    .map((e) => `${e.source}${e.branch !== null ? ` ${e.branch}` : ''} > ${e.target}`)
    .sort();
/// The step of the main path between two nodes.
const step = (chart: Chart, source: string, target: string) =>
  chart.edges.find((e) => e.kind === 'step' && e.source === source && e.target === target);
const isFork = (chart: Chart, id: string) => nodeOf(chart, id)?.kind === 'fork';
const isCondition = (chart: Chart, id: string) => ['fork', 'way-in'].includes(nodeOf(chart, id)?.kind ?? '');
/// The cells a fork's branches lead into.
const branchCells = (chart: Chart, fork: string) =>
  [...new Set(chart.edges.filter((e) => e.kind === 'step' && e.source === fork).map((e) => e.to))].sort();

describe('the flowchart of the engineer flow', () => {
  const chart = chartOf(engineer);

  it('has a node for each cell, in the order of the walk', () => {
    expect(chart.nodes.filter((n) => n.kind === 'cell').map((n) => n.cell)).toEqual(engineer.cells.map((c) => c.id));
  });

  it('draws one fork for each decision on deep_needed, named by the block, a branch for each value', () => {
    const forks = chart.nodes.filter((n) => n.kind === 'fork').map((n) => `${n.id} ${n.name}`);
    expect(forks).toEqual(['f:R1 deep_needed', 'f:R2 deep_needed', 'f:D4 edits_tree', 'f:T2 needs_testing']);
    // After R0: deep_needed may hold either, as a walk again after R1 wrote it finds it.
    expect(step(chart, cellNode('R0'), forkNode('R1'))).toBeDefined();
    expect(step(chart, forkNode('R1'), cellNode('R1'))?.branch).toBe('no');
    expect(step(chart, forkNode('R1'), cellNode('D1'))?.branch).toBe('yes');
    // After R1, which may write it: no into R2, yes into D1.
    expect(step(chart, cellNode('R1'), forkNode('R2'))).toBeDefined();
    expect(step(chart, forkNode('R2'), cellNode('R2'))?.branch).toBe('no');
    expect(step(chart, forkNode('R2'), cellNode('D1'))?.branch).toBe('yes');
    // A cell entered by a branch has no box of its own, and none where its when surely holds, as D2's after D1.
    for (const id of ['R1', 'R2', 'D1', 'D2', 'D3']) expect(nodeOf(chart, wayInNode(id))).toBeUndefined();
    expect(step(chart, cellNode('D1'), cellNode('D2'))?.branch).toBeNull();
    // R1 and D1 exclude each other as written; R1's own write of deep_needed is what parts them.
    const cellOf = (id: string) => engineer.cells.find((c) => c.id === id) as Cell;
    expect(exclusive(cellOf('R1'), cellOf('D1'))).toBe(true);
    expect(cellOf('R1').outputs).toContain('deep_needed');
  });

  it('draws no edge from one fork to another', () => {
    for (const e of chart.edges) expect(isFork(chart, e.source) && isFork(chart, e.target)).toBe(false);
  });

  it('ends no arrow at a condition but a way back into a fork whose branch cells it reaches, each of them', () => {
    for (const e of chart.edges.filter((x) => x.kind === 'arrow')) {
      if (!isCondition(chart, e.target)) {
        expect(e.target).toBe(cellNode(e.to));
        continue;
      }
      expect(e.back).toBe(true);
      expect(isFork(chart, e.target)).toBe(true);
      expect([...e.reaches].sort()).toEqual(branchCells(chart, e.target));
    }
    const into = chart.edges.filter((e) => e.kind === 'arrow' && isFork(chart, e.target));
    expect(into.map((e) => `${e.from}>${e.target} ${e.label} ${e.reaches.join(',')}`)).toEqual([
      'D4>f:R1 tree_review R1,D1',
      'I1>f:R1 plan_doubts R1,D1',
    ]);
    // A way back to one of a fork's cells alone ends at that cell.
    expect(arrow(chart, 'D3', 'D1')?.target).toBe(cellNode('D1'));
  });

  it('walks the main path cell to cell, parting at each fork and joining again', () => {
    expect(steps(chart)).toEqual(
      [
        'c:S1 > c:R0',
        'c:R0 > f:R1',
        'f:R1 no > c:R1',
        'f:R1 yes > c:D1',
        'c:R1 > f:R2',
        'f:R2 no > c:R2',
        'f:R2 yes > c:D1',
        // The short way skips D1 to D3, which run only where deep_needed is yes; the deep way goes through them.
        'c:R2 > f:D4',
        'c:D1 > c:D2',
        'c:D2 > c:D3',
        'c:D3 > f:D4',
        // The ways join at the fork on edits_tree: through D4 or past it, joining again at I1.
        'f:D4 yes > c:D4',
        'f:D4 no > c:I1',
        'c:D4 > c:I1',
        'c:I1 > c:T1',
        // T1 writes needs_testing: through T2 or past it, to M1's gate.
        'c:T1 > f:T2',
        'f:T2 yes > c:T2',
        'f:T2 no > w:M1',
        'c:T2 > w:M1',
        'c:M1 > w:M2',
      ].sort(),
    );
    const place = (id: string | null) => engineer.cells.findIndex((c) => c.id === id);
    for (const e of chart.edges.filter((x) => x.kind === 'step')) expect(place(e.from)).toBeLessThanOrEqual(place(e.to));
  });

  it('keeps each requires as a gate on the path into its cell, with what the outside writes of it', () => {
    expect(nodeOf(chart, wayInNode('M1'))?.requires.map((k) => (k.on === 'test' ? k.test.name : k.block))).toEqual([
      'settled',
      'asked',
      'review_ok',
    ]);
    expect(nodeOf(chart, wayInNode('M2'))?.requires).toEqual([{ on: 'block', block: 'checked', value: 'yes' }]);
    expect(nodeOf(chart, wayInNode('S1'))?.requires).toEqual([{ on: 'block', block: 'dependencies', value: 'merged' }]);
    expect(chart.nodes.filter((n) => n.kind === 'way-in').map((n) => n.cell)).toEqual(['S1', 'M1', 'M2']);
    for (const id of ['S1', 'M1', 'M2']) {
      expect(chart.edges.find((e) => e.id === `in:${id}`)).toMatchObject({
        source: wayInNode(id),
        target: cellNode(id),
        main: true,
      });
    }
    expect(step(chart, cellNode('T2'), wayInNode('M1'))).toBeDefined();
    expect(step(chart, forkNode('T2'), wayInNode('M1'))).toBeDefined();
    const tag = chart.edges.find((e) => e.kind === 'outside' && e.target === wayInNode('S1'));
    expect(nodeOf(chart, tag?.source ?? '')?.name).toBe('dependencies');
    expect(nodeOf(chart, tag?.source ?? '')?.outside).toEqual({ people: false, sources: ['dependencies'] });
    // An arrow a write runs ends at the cell itself, not at its gate.
    expect(arrow(chart, 'I1', 'M1')?.target).toBe(cellNode('M1'));
  });

  it('leaves out an arrow between two cells whose conditions exclude each other', () => {
    const all = arrows(chart);
    // The plan of the short way does not run the deep way's cells, nor the other way round.
    for (const pair of ['R2>D2', 'R2>D3', 'D1>R2', 'D2>R2']) expect(all[pair]).toBeUndefined();
    // The review of the deep way does not send the work back to the short way's research.
    expect(all['D3>R1']).toBeUndefined();
  });

  it('draws one arrow for each pair, with the name of every trigger it stands for', () => {
    const all = arrows(chart);
    expect(all['R1>D3']).toBe('plan, reply_plan_review');
    expect(all['I1>T1']).toBe('change, reply_review');
    expect(arrow(chart, 'I1', 'T1')?.carries).toEqual([
      { name: 'change', group: true },
      { name: 'reply_review', group: false },
    ]);
    expect(all['I1>T2']).toBe('acceptance, reply_testing');
    expect(all['I1>M1']).toBe('code, reply_check');
    expect(all['M1>M2']).toBe('checked');
    const pairs = everyArrow(chart).map((e) => `${e.from}>${e.to}`);
    expect(new Set(pairs).size).toBe(pairs.length);
    // Two ways back are drawn as one each, into a fork (below).
    expect(pairs).toHaveLength(36);
  });

  it('folds an arrow that runs along the main path into its step, its names on the step after the branch', () => {
    // R1's plan runs R2, which the path reaches from R1 through the fork alone: on the branch no, not a second line.
    const r2 = step(chart, forkNode('R2'), cellNode('R2'));
    expect(`${r2?.branch} ${r2?.label}`).toBe('no plan');
    expect(r2?.folds.map((f) => f.id)).toEqual(['a:R1>R2']);
    expect(drawn(chart)).not.toContain('R1>R2');
    const d1 = step(chart, forkNode('R2'), cellNode('D1'));
    expect(`${d1?.branch} ${d1?.label}`).toBe('yes found');
    // R2's plan runs I1, which the path reaches from R2 past D4 alone.
    expect(step(chart, forkNode('D4'), cellNode('I1'))?.folds.map((f) => f.id)).toEqual(['a:R2>I1']);
    expect(step(chart, cellNode('I1'), cellNode('T1'))?.label).toBe('change, reply_review');
    expect(step(chart, cellNode('T2'), wayInNode('M1'))?.label).toBe('tests');
    // A step no arrow runs along carries nothing.
    expect(step(chart, cellNode('D3'), forkNode('D4'))?.label).toBe('');
    // An arrow with a cell of the path between stays an arrow: R1's plan runs D3, but D1 and D2 come between.
    expect(drawn(chart).filter((pair) => !chart.edges.some((e) => e.back && `${e.from}>${e.to}` === pair))).toEqual([
      'R1>D2',
      'R1>D3',
      'R1>D4',
      'R1>I1',
      'R1>T2',
      'R2>T2',
      'D1>D3',
      'D1>D4',
      'D1>I1',
      'D1>T2',
      'D2>D4',
      'D2>I1',
      'D2>T2',
      'I1>T2',
      'I1>M1',
    ]);
    // A way back is never folded.
    expect(chart.edges.flatMap((e) => e.folds).filter((f) => f.back)).toEqual([]);
  });

  it('draws an arrow back up the walk as a way back, into a cell, or into a fork for every cell it leads to', () => {
    const back = chart.edges.filter((e) => e.back).map((e) => `${e.from}>${e.target} ${e.label}`);
    expect(back).toEqual([
      'D4>f:R1 tree_review',
      'I1>f:R1 plan_doubts',
      'D3>c:D1 plan_review',
      'I1>c:R2 plan',
      'I1>c:D2 plan',
      'I1>c:D3 plan',
      'I1>c:D4 plan',
      'T1>c:I1 review',
      'T2>c:I1 testing',
      'T2>c:T1 change',
      'M1>c:I1 check',
    ]);
    expect(chart.waysBack).toBe(11);
    const place = (id: string | null) => engineer.cells.findIndex((c) => c.id === id);
    for (const e of chart.edges.filter((x) => x.kind === 'arrow')) expect(e.back).toBe(place(e.from) > place(e.to));
  });

  it('draws what people and the outside write for a cell as tags beside it, each into it', () => {
    const tags = (id: string) =>
      chart.edges
        .filter((e) => e.kind === 'outside' && e.target === cellNode(id))
        .map((e) => nodeOf(chart, e.source))
        .map((n) => `${n?.name} ${n?.outside?.people ? 'people' : ''}${n?.outside?.sources.join(' ') ?? ''}`);
    expect(tags('R1')).toEqual(['background people', 'r1_spoken_to people', 'r1_answered answer']);
    expect(tags('D1')).toEqual([
      'background people',
      'deep_researcher_session session',
      'd1_spoken_to people',
      'd1_answered answer',
    ]);
    expect(tags('M1')).toEqual(['main main', 'asked asked']);
    expect(tags('R2')).toEqual(['r2_answered answer']);
    expect(nodeOf(chart, outsideNode('R0', 'engineer_session'))?.cell).toBe('R0');
  });

  it('needs no tie where the main path reaches every cell', () => {
    expect(chart.edges.filter((e) => e.kind === 'tie')).toEqual([]);
  });

  it('draws the arrows a write runs into and out of the cell chosen, or every one', () => {
    const pairs = (chosen: string | null, all: boolean) => shownArrows(chart, chosen, all).map((e) => `${e.from}>${e.to}`);
    expect(pairs('D1', false)).toEqual(['D1>D3', 'D1>D4', 'D1>I1', 'D1>T2']);
    expect(pairs('T2', false)).toEqual(['R1>T2', 'R2>T2', 'D1>T2', 'D2>T2', 'I1>T2']);
    // None with nothing chosen, nor the folded ones nor the ways back of the cell chosen.
    expect(pairs(null, false)).toEqual([]);
    expect(pairs('R2', false)).toEqual(['R2>T2']);
    expect(pairs(null, true)).toHaveLength(15);
  });

  it('gives the context of a cell in the order it names it, and lights the cells that write it', () => {
    const context = contextOf(engineer, 'I1');
    expect(context.map((t) => `${t.block} ${t.kind}`)).toEqual([
      'background latest',
      'plan_owner lastAndLatest',
      'plan_agent lastAndLatest',
      'questions latest',
      'answers all',
      'acceptance lastAndLatest',
      'tree_edits lastAndLatest',
      'reply_plan_doubts latest',
      'discussion all',
      'review latest',
      'testing latest',
      'check latest',
      'plan_doubts latest',
      'tree lastAndLatest',
    ]);
    expect(context.find((t) => t.block === 'tree')?.outside).toEqual({ people: false, sources: ['tree'] });
    expect([...litBy(engineer, 'I1')].sort()).toEqual(['D1', 'D2', 'D3', 'D4', 'M1', 'R1', 'R2', 'T1', 'T2']);
  });
});

describe('the flowchart of a small flow', () => {
  // A letter is sorted: what is urgent is rushed, the rest queued, either way drafted and polished; a review may send
  // the copy back; it is sent once approved, and the files tidied on the clock.
  const approved = test('approved', 'The review found it fine', 'all', ['fine']);
  const flow = (): Flow =>
    flowOf(
      [
        cell('sort', {
          line: 'clerk',
          when: [{ on: 'block', block: 'urgent', value: 'no' }],
          runsOn: ['letter'],
          outputs: ['urgent', 'summary'],
        }),
        cell('rush', {
          line: 'clerk',
          when: [{ on: 'block', block: 'urgent', value: 'yes' }],
          runsOn: ['summary'],
          outputs: ['draft'],
        }),
        cell('queue', {
          line: 'sorter',
          when: [{ on: 'block', block: 'urgent', value: 'no' }],
          runsOn: ['summary', 'draft'],
          outputs: ['draft', 'note'],
        }),
        cell('polish', {
          line: 'editor',
          runsOn: ['draft', 'note', 'remarks'],
          given: ['summary', 'remarks'],
          triggers: [
            { name: 'on_copy', what: null, block: 'copy', group: ['draft', 'note'], read: [], do: null },
            { name: 'on_remarks', what: null, block: 'remarks', group: [], read: [], do: null },
          ],
          input: {
            summary: { runs: null, given: { kind: 'latest', why: null } },
            remarks: { runs: { why: null }, given: { kind: 'all', why: null } },
          },
          outputs: ['final'],
        }),
        cell('critic', { line: 'reviewer', runsOn: ['final'], outputs: ['remarks', 'fine'] }),
        cell('send', {
          kind: 'engine',
          line: null,
          act: 'merge',
          requires: tested(approved),
          runsOn: ['final', 'clock'],
          outputs: ['sent'],
        }),
        cell('tidy', { kind: 'engine', line: null, act: 'tidy', runsOn: ['clock'] }),
      ],
      {
        letter: { people: true },
        urgent: { values: ['yes', 'no'] },
        summary: {},
        draft: {},
        note: {},
        remarks: {},
        final: {},
        fine: { values: ['yes', 'no'], people: true },
        clock: { outside: 'clock' },
        sent: { shown: true },
      },
      'letters',
    );

  it('has a node for each cell in the order of the walk, a fork before each it parts on and a gate before each requires', () => {
    const chart = chartOf(flow());
    expect(chart.nodes.filter((n) => n.kind === 'cell').map((n) => n.cell)).toEqual([
      'sort',
      'rush',
      'queue',
      'polish',
      'critic',
      'send',
      'tidy',
    ]);
    expect(chart.nodes.filter((n) => n.kind === 'fork').map((n) => `${n.id} ${n.name}`)).toEqual([
      'f:sort urgent',
      'f:rush urgent',
    ]);
    expect(chart.nodes.filter((n) => n.kind === 'way-in').map((n) => n.cell)).toEqual(['send']);
    // A gate stands just before its cell.
    const ids = chart.nodes.map((n) => n.id);
    expect(ids.indexOf(wayInNode('send'))).toBe(ids.indexOf(cellNode('send')) - 1);
  });

  it('parts once on a block for each of its values, one fork, the cells that share the block its branches', () => {
    const chart = chartOf(flow());
    expect(steps(chart)).toEqual(
      [
        // urgent may hold either, or nothing before sort writes it: sort when no, rush when yes, else polish.
        'f:sort no > c:sort',
        'f:sort yes > c:rush',
        'f:sort > c:polish',
        // Once sort has written it, again: rush when yes, queue when no.
        'c:sort > f:rush',
        'f:rush yes > c:rush',
        'f:rush no > c:queue',
        'c:rush > c:polish',
        'c:queue > c:polish',
        'c:polish > c:critic',
        'c:critic > w:send',
        'c:send > c:tidy',
      ].sort(),
    );
    // rush and queue never run in one work, and rush writes no urgent: its draft does not run queue.
    expect(arrow(chart, 'rush', 'queue')).toBeUndefined();
  });

  it('draws an arrow for each write a trigger runs into the cell it runs, along the path folded into it', () => {
    const chart = chartOf(flow());
    expect(arrows(chart)).toEqual({
      'sort>rush': 'summary',
      'sort>queue': 'summary',
      'rush>polish': 'copy',
      'queue>polish': 'copy',
      'polish>critic': 'final',
      'polish>send': 'final',
      'critic>polish': 'remarks',
    });
    expect(drawn(chart)).toEqual(['polish>send', 'critic>polish']);
    expect(arrow(chart, 'polish', 'send')?.target).toBe(cellNode('send'));
    expect(step(chart, forkNode('rush'), cellNode('rush'))?.label).toBe('summary');
    expect(arrow(chart, 'rush', 'polish')?.carries).toEqual([{ name: 'copy', group: true }]);
  });

  it('draws the way back up the walk and counts it', () => {
    const chart = chartOf(flow());
    expect(chart.edges.filter((e) => e.back).map((e) => `${e.from}>${e.to}`)).toEqual(['critic>polish']);
    expect(chart.waysBack).toBe(1);
  });

  it('puts the requires of a cell before it as a gate, and what people and the outside write beside the cells', () => {
    const chart = chartOf(flow());
    expect(nodeOf(chart, wayInNode('send'))?.requires).toEqual(tested(approved));
    // fine is written by a cell as well as by people: nothing outside is drawn for the gate.
    expect(chart.edges.filter((e) => e.kind === 'outside' && e.target === wayInNode('send'))).toEqual([]);
    expect(chart.edges.filter((e) => e.kind === 'outside').map((e) => `${e.source}>${e.target}`)).toEqual([
      `${outsideNode('sort', 'letter')}>${cellNode('sort')}`,
      `${outsideNode('send', 'clock')}>${cellNode('send')}`,
      `${outsideNode('tidy', 'clock')}>${cellNode('tidy')}`,
    ]);
    expect(nodeOf(chart, outsideNode('sort', 'letter'))?.outside).toEqual({ people: true, sources: [] });
    expect(nodeOf(chart, outsideNode('tidy', 'clock'))?.outside).toEqual({ people: false, sources: ['clock'] });
  });

  it('gives what a cell is given with its kind, and lights its writers', () => {
    expect(contextOf(flow(), 'polish')).toEqual([
      { block: 'summary', kind: 'latest', writers: ['sort'], outside: null },
      { block: 'remarks', kind: 'all', writers: ['critic'], outside: null },
    ]);
    expect([...litBy(flow(), 'polish')]).toEqual(['sort', 'critic']);
    expect(contextOf(flow(), 'tidy')).toEqual([]);
  });
});

describe('the main path of a bakery', () => {
  // The oven is lit when it burns wood, which lighting it may turn to gas; the dough is kneaded and made bread, a
  // cake or a pie, or none of them, each by a cell of its own; it is baked once the oven is hot, glazed where people
  // want it shiny, and boxed; a complaint about the box sends it back to the kneading. The floor is swept where it was
  // mopped, which nothing writes.
  const hot = test('hot', 'The oven is hot', 'all', ['oven']);
  const kind = (value: string) => [{ on: 'block' as const, block: 'kind', value }];
  const bakery = (): Flow => {
    const flow = flowOf(
      [
        cell('light', {
          line: 'stoker',
          when: [{ on: 'block', block: 'fuel', value: 'wood' }],
          runsOn: ['order'],
          outputs: ['fuel', 'oven'],
        }),
        cell('knead', { line: 'baker', runsOn: ['order', 'complaint'], outputs: ['dough', 'kind'] }),
        cell('proof', { line: 'baker', when: kind('bread'), runsOn: ['dough'], outputs: ['risen'] }),
        cell('whisk', { line: 'baker', when: kind('cake'), runsOn: ['dough'], outputs: ['batter'] }),
        cell('roll', { line: 'baker', when: kind('pie'), runsOn: ['dough'], outputs: ['crust'] }),
        cell('bake', {
          kind: 'engine',
          line: null,
          act: 'bake',
          requires: tested(hot),
          runsOn: ['dough', 'risen', 'batter', 'crust'],
          outputs: ['loaf'],
        }),
        cell('glaze', {
          line: 'decorator',
          when: [{ on: 'block', block: 'shiny', value: 'yes' }],
          runsOn: ['loaf'],
          outputs: ['loaf'],
        }),
        cell('sweep', { line: 'cleaner', when: [{ on: 'block', block: 'mopped', value: 'yes' }], runsOn: ['broom'] }),
        cell('box', { line: 'packer', runsOn: ['loaf', 'dough'], outputs: ['complaint'] }),
      ],
      {
        order: { people: true },
        fuel: { values: ['wood', 'gas'] },
        oven: { values: ['yes', 'no'] },
        dough: {},
        kind: { values: ['bread', 'cake', 'pie'] },
        risen: {},
        batter: {},
        crust: {},
        loaf: {},
        shiny: { values: ['yes', 'no'], people: true },
        complaint: {},
        mopped: { values: ['yes', 'no'] },
        broom: { outside: 'clock' },
      },
      'bakery',
    );
    const defaults: Record<string, string> = { fuel: 'wood', shiny: 'no' };
    return { ...flow, blocks: flow.blocks.map((b) => ({ ...b, default: defaults[b.name] ?? null })) };
  };

  it('parts on each value of a choice at one fork, and joins again where the branches meet', () => {
    const chart = chartOf(bakery());
    expect(steps(chart)).toEqual(
      [
        // fuel may be gas once light has written it, as a walk again finds it: light when wood, else on to knead.
        'f:light wood > c:light',
        'f:light gas > c:knead',
        'c:light > c:knead',
        // knead may write any kind, or none: one fork on kind, a branch for each value, joining at bake's gate.
        'c:knead > f:proof',
        'f:proof bread > c:proof',
        'f:proof cake > c:whisk',
        'f:proof pie > c:roll',
        'f:proof > w:bake',
        'c:proof > w:bake',
        'c:whisk > w:bake',
        'c:roll > w:bake',
        // People may want it shiny, whatever its default: through glaze or past it, joining at box.
        'c:bake > f:glaze',
        'f:glaze yes > c:glaze',
        'f:glaze no > c:box',
        'c:glaze > c:box',
      ].sort(),
    );
    expect(chart.nodes.filter((n) => n.kind === 'fork').map((n) => `${n.id} ${n.name}`)).toEqual([
      'f:light fuel',
      'f:proof kind',
      'f:glaze shiny',
      'f:sweep null',
    ]);
    // bake's requires is a gate, not a fork: the path waits at it and goes into bake, and leaves it by no other way.
    expect(chart.edges.find((e) => e.id === 'in:bake')?.main).toBe(true);
    expect(chart.edges.filter((e) => e.kind === 'step' && e.source === wayInNode('bake'))).toEqual([]);
  });

  it('draws the condition of a cell the path never reaches before it, kept in the order of the walk by a tie', () => {
    const chart = chartOf(bakery());
    // Nothing writes mopped, which has no default: sweep never runs, and is on no way of the path.
    expect(chart.edges.filter((e) => e.kind === 'step' && e.to === 'sweep')).toEqual([]);
    expect(nodeOf(chart, forkNode('sweep'))?.when).toEqual([{ on: 'block', block: 'mopped', value: 'yes' }]);
    expect(chart.edges.find((e) => e.id === `in:${forkNode('sweep')}`)).toMatchObject({ target: cellNode('sweep'), main: false });
    expect(chart.edges.filter((e) => e.kind === 'tie').map((e) => `${e.source}>${e.target}`)).toEqual([
      `${cellNode('glaze')}>${forkNode('sweep')}`,
    ]);
  });

  it('folds each arrow along the path into the step that enters the cell it runs, a cell between keeping it apart', () => {
    const chart = chartOf(bakery());
    const folds = (source: string, target: string) => step(chart, source, target)?.folds.map((f) => f.id);
    // Straight from the cell that writes to the cell it runs, or through a fork alone, on the branch into the cell.
    expect(folds(forkNode('proof'), cellNode('proof'))).toEqual(['a:knead>proof']);
    expect(folds(forkNode('proof'), cellNode('whisk'))).toEqual(['a:knead>whisk']);
    expect(folds(forkNode('proof'), cellNode('roll'))).toEqual(['a:knead>roll']);
    expect(folds(forkNode('proof'), wayInNode('bake'))).toEqual(['a:knead>bake']);
    expect(folds(cellNode('proof'), wayInNode('bake'))).toEqual(['a:proof>bake']);
    expect(folds(cellNode('glaze'), cellNode('box'))).toEqual(['a:glaze>box']);
    expect(folds(forkNode('glaze'), cellNode('box'))).toEqual(['a:bake>box']);
    const pie = step(chart, forkNode('proof'), cellNode('roll'));
    expect(`${pie?.branch} ${pie?.label}`).toBe('pie dough');
    // A step nothing runs along carries nothing.
    expect(step(chart, cellNode('light'), cellNode('knead'))?.label).toBe('');
    // With bake between, knead's dough into box stays an arrow; the complaint back to knead is a way back.
    expect(drawn(chart)).toEqual(['knead>box', 'box>knead']);
    expect(arrow(chart, 'box', 'knead')?.back).toBe(true);
  });
});
