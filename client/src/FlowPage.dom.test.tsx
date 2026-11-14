// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { type Block, type Definition, type Given, type SentPart, type SentPiece, load } from './api';
import { FlowPage, Shell } from './App';
import { cell, flowOf, small, test, tested } from './testing';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// Draws the page for this definition, with what an agent is sent where it is given, and gives its root.
function draw(definition: Definition, given?: Given): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<FlowPage definition={definition} given={given} />));
  unmount = () => act(() => root.unmount());
  return container;
}

const texts = (root: HTMLElement, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

/// The words of each item of the lists in a part of a panel, each item's parts joined by one space.
const items = (root: HTMLElement) =>
  [...root.querySelectorAll('.uses li')].map((li) =>
    [...li.children]
      .map((c) => c.textContent?.trim())
      .join(' ')
      .replace(/\s+/g, ' '),
  );

function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  act(() => (element as HTMLElement).click());
}

/// What the buttons of the cells' heads are called: each cell's id, title, line, and when it runs.
const headNames = (page: HTMLElement) =>
  [...page.querySelectorAll('thead .cell-head .head-button')].map((b) => b.getAttribute('aria-label'));

/// The rows of the triggers, in their band, or of the data, under the bands.
const rows = (page: HTMLElement, triggers: boolean) => [
  ...page.querySelectorAll(triggers ? 'tbody.triggers tr' : 'tbody:not(.band) tr'),
];

/// The row of a datum, or of a trigger, of this name.
const rowOf = (page: HTMLElement, name: string, trigger = false) =>
  rows(page, trigger).find((tr) => tr.querySelector('.block-name')?.textContent === name) as HTMLElement;

/// The row of a band over the blocks, by its kind (`line-row`, `condition-row`) and its name.
const bandRow = (page: HTMLElement, kind: string, name: string) =>
  [...page.querySelectorAll(`tbody.band tr.${kind}`)].find(
    (tr) => tr.querySelector('.block-name')?.textContent === name,
  ) as HTMLElement;

describe('the chart', () => {
  it('shows the triggers’ rows over the data’s, the cells across in the order of the walk, and what each cell does with each', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'thead .cell-head .cell-id')).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(rows(page, true).map((tr) => tr.querySelector('.block-name')?.textContent)).toEqual([
      'on_request',
      'on_plan',
      'on_opinions',
      'on_design',
      'on_comments',
      'on_main',
      'on_ci',
    ]);
    expect(rows(page, false).map((tr) => tr.querySelector('.block-name')?.textContent)).toEqual([
      'request',
      'plan',
      'needs_design',
      'opinions',
      'design',
      'commented',
      'confirmed',
      'comments',
      'reply',
      'main',
      'ci',
    ]);
    // On a trigger's row, the cells it runs and those that write the block it stands on; on a datum's, those given it
    // and those that write it.
    expect(texts(rowOf(page, 'on_plan', true), '.spot .visually-hidden')).toEqual([
      'plan outputs plan',
      'judge triggered_by on_plan',
      'design triggered_by on_plan',
      'answer triggered_by on_plan',
    ]);
    expect(texts(rowOf(page, 'plan'), '.spot .visually-hidden')).toEqual([
      'plan outputs plan',
      'judge context plan',
      'design context plan',
      'answer context plan',
    ]);
    // What its condition reads is on the condition's own row.
    expect(texts(rowOf(page, 'needs_design'), '.spot .visually-hidden')).toEqual(['judge outputs needs_design']);
    expect(rowOf(page, 'needs_design').querySelector('.block-values')?.textContent).toBe('yes|no');
    // A head holds the id alone; its title, line, when and requires are what its button is called, by their keys.
    expect(texts(page, 'thead .cell-head .head-button')).toEqual([
      'plan',
      'judge',
      'design',
      'answer',
      'check',
      'merge',
    ]);
    expect(headNames(page)[2]).toBe('design Design, engineer, when needs_design = yes');
    expect(headNames(page)[3]).toBe(
      'answer Answer comments, engineer, when test = has_comment, requires test = plan_confirmed',
    );
    // The one way back: judge writes the opinions whose trigger runs plan, and the arrow is at plan's mark.
    expect(page.querySelector('.legend')?.textContent).toContain('way back (1)');
    const arrows = [...page.querySelectorAll('.arrow')];
    expect(arrows).toHaveLength(1);
    expect(arrows[0].closest('td')?.textContent).toBe('plan triggered_by on_opinions');
  });

  it('draws each condition once, on its own row over the blocks: a block a when reads, or a named test', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'tbody.band .condition-row .block-name')).toEqual([
      'needs_design',
      'has_comment',
      'plan_confirmed',
      'may_merge',
    ]);
    expect(texts(bandRow(page, 'condition-row', 'needs_design'), '.spot .visually-hidden')).toEqual([
      'design when needs_design = yes',
    ]);
    expect(texts(bandRow(page, 'condition-row', 'has_comment'), '.spot .visually-hidden')).toEqual([
      'answer when test = has_comment',
    ]);
    expect(texts(bandRow(page, 'condition-row', 'may_merge'), '.spot .visually-hidden')).toEqual([
      'merge requires test = may_merge',
    ]);
    // What a test says is in the column of words, whether a cell's when names it or its requires.
    expect(bandRow(page, 'condition-row', 'has_comment').querySelector('.words')?.textContent).toBe(
      'The item has a comment',
    );
    expect(bandRow(page, 'condition-row', 'plan_confirmed').querySelector('.words')?.textContent).toBe(
      'The owner confirmed the plan',
    );
    // Nothing of the conditions stays on the blocks' rows.
    expect(texts(rowOf(page, 'commented'), '.spot .visually-hidden')).toEqual([]);
    expect(texts(rowOf(page, 'ci'), '.spot .visually-hidden')).toEqual(['check outputs ci']);
    expect(texts(rowOf(page, 'on_ci', true), '.spot .visually-hidden')).toEqual([
      'check outputs ci',
      'merge triggered_by on_ci',
    ]);
    expect(page.querySelectorAll('tbody:not(.band) .part.condition')).toHaveLength(0);
    expect(page.querySelectorAll('tbody:not(.band) .part.precondition')).toHaveLength(0);
    expect(bandRow(page, 'condition-row', 'may_merge').querySelectorAll('.part.precondition')).toHaveLength(1);
    expect(bandRow(page, 'condition-row', 'needs_design').querySelectorAll('.part.holds')).toHaveLength(1);
    expect(page.querySelector('.legend')?.textContent).toContain('when no');
  });

  it('marks a when on a block by a tick for yes, a cross for no and the value itself for another', () => {
    const flow = flowOf(
      [
        cell('a', { outputs: ['x'] }),
        cell('b', { when: [{ on: 'block', block: 'x', value: 'yes' }] }),
        cell('c', { when: [{ on: 'block', block: 'x', value: 'no' }] }),
        cell('d', { when: [{ on: 'block', block: 'x', value: 'maybe' }] }),
        cell('e', { kind: 'engine', line: null, act: 'merge', task: null }),
      ],
      { x: { values: ['yes', 'no', 'maybe'], default: 'no' } },
    );
    const page = draw({ state: 'compiled', flow });
    const x = bandRow(page, 'condition-row', 'x');
    const spots = [...x.querySelectorAll('td.spot')];
    expect(spots[1].querySelector('.part.holds')?.getAttribute('d')).toBe('M5.5 12.5 L10 17 L18.5 7');
    expect(spots[2].querySelector('.part.holds')?.getAttribute('d')).toBe('M7 7 L17 17 M17 7 L7 17');
    expect(spots[3].querySelector('.spot-value')?.textContent).toBe('maybe');
    // What a block counts as before it is written is in the column of words, by its key.
    expect(x.querySelector('.words')?.textContent).toBe('default no');
    click(page.querySelectorAll('thead .cell-head .head-button')[2]);
    expect(x.querySelector('.words')?.textContent).toBe('when x = no');
  });

  it('opens a named test’s panel, with its file and its keys as written, from its row', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(bandRow(page, 'condition-row', 'has_comment').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Test has_comment"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('tests/has_comment.toml');
    expect(texts(panel, '.status-line')).toEqual([
      'saysThe item has a comment',
      'kind any',
      'reads [commented]',
      'is yes',
    ]);
    expect(texts(panel, '.panel-part-label')).toEqual(['when 1']);
  });

  it('says a trigger’s marks by the trigger and by the block it stands on, and chooses that block by its row', () => {
    // The trigger y stands on the block x; y is also a datum of its own.
    const trigger = { name: 'y', what: 'Fires when x is new.', block: 'x', group: [], read: [], do: 'Read x again.' };
    const flow = flowOf(
      [cell('a', { outputs: ['x', 'y'] }), cell('b', { runsOn: ['x'], given: ['y'], triggers: [trigger] })],
      { x: { what: 'The plan.' }, y: { what: 'The notes.' } },
    );
    const page = draw({ state: 'compiled', flow });
    const row = rowOf(page, 'y', true);
    expect(texts(row, '.spot .visually-hidden')).toEqual(['a outputs x', 'b triggered_by y']);
    expect(row.querySelector('.head-button')?.getAttribute('aria-label')).toBe('Trigger y, block x');
    expect(row.querySelector('.words')?.textContent).toBe('Fires when x is new.');
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    expect(row.querySelector('.words')?.textContent).toBe('Read x again.');
    click(row.querySelector('.head-button'));
    expect(page.querySelector('aside[aria-label="Block x"]')).not.toBeNull();
    expect(rowOf(page, 'x').className).toContain('chosen');
    // The datum y is chosen by its own row, not by the trigger of its name.
    click(rowOf(page, 'y').querySelector('.head-button'));
    expect(page.querySelector('aside[aria-label="Block y"]')).not.toBeNull();
    expect(row.className).not.toContain('chosen');
  });

  it('signs the source outside that writes a block on its row, and says it in its panel by its key', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(rowOf(page, 'main').querySelector('[title="from main"]')).not.toBeNull();
    expect(rowOf(page, 'plan').querySelector('[title^="from "]')).toBeNull();
    click(rowOf(page, 'main').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block main"]') as HTMLElement;
    expect(texts(panel, '.status-line')).toContain('written_byoutside main');
  });

  it('shows everything about a cell chosen by its column’s head, and about a block chosen from it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(panel.textContent).toContain('line engineer cells plan, design, answer');
    // What runs it, by the names of the triggers its file writes, each with the block it stands on.
    expect(items(panel).slice(0, 6)).toEqual([
      '1 on_request block request written_by people',
      '2 on_opinions block opinions written_by judge, people way_back [judge]',
      '1 task',
      '2 request kind written_by people',
      '3 opinions kind written_by judge, people',
      'plan read_by cells judge, design, answer',
    ]);
    click([...panel.querySelectorAll('button.link')].find((b) => b.textContent === 'opinions'));
    const block = page.querySelector('aside[aria-label="Block opinions"]') as HTMLElement;
    expect(items(block)).toEqual(['judge → plan triggered_by [on_opinions]']);
  });

  it('shows an agent’s cell’s task with its line breaks, and the act that carries out an engine’s cell', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const heads = page.querySelectorAll('thead .cell-head .head-button');
    click(heads[0]);
    const plan = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(texts(plan, '.panel-part-label')).toEqual(['triggered_by 2', 'context 3', 'outputs 1', 'task', 'line']);
    expect(plan.querySelector('.content')?.textContent).toBe('Write the plan from the request.\nSay what it leaves out.');
    click(heads[4]);
    const check = page.querySelector('aside[aria-label="Cell check"]') as HTMLElement;
    expect(texts(check, '.panel-part-label')).toEqual(['triggered_by 2', 'outputs 1', 'act']);
    const act = [...check.querySelectorAll('.panel-part')].find((p) => p.querySelector('.panel-part-label')?.textContent === 'act');
    expect(act?.querySelector('.mono')?.textContent).toBe('check');
  });

  it('marks among what a cell may write the blocks it writes every time, by their key', () => {
    const flow = flowOf([cell('plan', { inputs: ['opinions'] }), cell('judge', { outputs: ['opinions', 'verdict'], mustWrite: ['verdict'] })], {
      opinions: {},
      verdict: { values: ['yes', 'no'], shown: true },
    });
    const page = draw({ state: 'compiled', flow });
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    const panel = page.querySelector('aside[aria-label="Cell judge"]') as HTMLElement;
    const outputs = [...panel.querySelectorAll('.panel-part')].find((p) =>
      p.querySelector('.panel-part-label')?.textContent?.startsWith('outputs'),
    ) as HTMLElement;
    expect(items(outputs)).toEqual(['opinions read_by cells plan way_back [plan]', 'verdict must_write true read_by shown true']);
  });

  it('shows everything about a block chosen by its row’s name', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(rowOf(page, 'ci').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block ci"]') as HTMLElement;
    expect(panel.textContent).toContain('values [green, red]');
    expect(texts(panel, '.status-line')).toContain('read_bycells merge tests may_merge');
    expect(panel.querySelector('[title="kind none, reads [ci], is red"]')?.textContent).toBe('may_merge');
    expect(rowOf(page, 'ci').className).toContain('chosen');
  });

  it('gives each cell’s column an edge to drag', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(page.querySelectorAll('thead .cell-head .edge')).toHaveLength(6);
  });
});

describe('the column of words', () => {
  const flow = () =>
    flowOf(
      [
        cell('a', { title: 'First', outputs: ['x', 'y'], mustWrite: ['x'] }),
        cell('b', {
          title: 'Second',
          when: [{ on: 'block', block: 'y', value: 'yes' }],
          inputs: ['x', 'said'],
          input: { x: { runs: { why: null }, given: { kind: 'latest', why: 'It is what to judge.' } } },
          outputs: ['said'],
        }),
      ],
      { x: { what: 'The plan.' }, y: { values: ['yes', 'no'] }, said: { people: true, what: 'What was said of it.' } },
    );
  /// The words of each datum's row, by its block.
  const words = (page: HTMLElement) =>
    Object.fromEntries(
      rows(page, false).map((tr) => [tr.querySelector('.block-name')?.textContent, tr.querySelector('.words')?.textContent]),
    );

  it('says what each block is, and of the cell chosen what its file says of each block it has to do with', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    expect(words(page)).toEqual({ x: 'The plan.', y: '', said: 'What was said of it.' });
    expect(page.querySelector('.words-label')?.textContent).toBe('What each row is');
    expect(page.querySelector('.words-about')?.textContent).toBe('');

    const heads = page.querySelectorAll('thead .cell-head .head-button');
    click(heads[1]);
    expect(words(page)).toEqual({
      x: 'triggered_by on_x; context latest: It is what to judge.',
      // Its when is said on the condition's row, once.
      y: '',
      said: 'triggered_by on_said; context; outputs: What was said of it.',
    });
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each row');
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer · when y = yes');

    click(heads[0]);
    expect(words(page)).toEqual({ x: 'must_write: The plan.', y: 'outputs', said: 'What was said of it.' });
    expect(rowOf(page, 'said').querySelector('.words')?.className).toContain('beside');
  });

  it('says what a block is in the block’s panel too, by its key', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    click(rowOf(page, 'x').querySelector('.head-button'));
    expect(texts(page.querySelector('aside[aria-label="Block x"]') as HTMLElement, '.status-line')).toContain(
      'whatThe plan.',
    );
    click(rowOf(page, 'y').querySelector('.head-button'));
    const y = page.querySelector('aside[aria-label="Block y"]') as HTMLElement;
    expect(texts(y, '.status-line').some((line) => line.startsWith('what'))).toBe(false);
  });

  it('names over the words the cell under the pointer, and else the cell chosen', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    const heads = page.querySelectorAll('thead .cell-head .head-button');
    const point = (at: Element | null) => act(() => void at?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })));
    click(heads[1]);
    point(heads[0]);
    expect(page.querySelector('.words-about')?.textContent).toBe('a First · engineer');
    // The rows still say what the chosen cell says.
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each row');
    point(page.querySelector('.words-head'));
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer · when y = yes');
  });
});

describe('a test of what stands', () => {
  it('is named by its key in the head of the cell that waits on it, and in its panel with what it says', () => {
    const approved = test('approved', 'No judge refuses', 'noneStanding', ['verdict', 'deep_verdict'], 'no');
    const flow = flowOf(
      [
        cell('judge', { outputs: ['verdict', 'deeper'] }),
        cell('deep', { when: [{ on: 'block', block: 'deeper', value: 'yes' }], outputs: ['deep_verdict'] }),
        cell('merge', { kind: 'engine', line: null, effort: null, requires: tested(approved) }),
      ],
      { verdict: { values: ['yes', 'no'] }, deeper: { values: ['yes', 'no'] }, deep_verdict: { values: ['yes', 'no'] } },
    );
    const page = draw({ state: 'compiled', flow });
    expect(headNames(page)[2]).toBe('merge merge, engine, requires test = approved');
    click(page.querySelectorAll('thead .cell-head .head-button')[2]);
    expect(texts(page.querySelector('aside[aria-label="Cell merge"]') as HTMLElement, '.status-line')).toContain(
      'requires test = approvedNo judge refuses',
    );
  });
});

describe('a when or a requires of several keys', () => {
  it('draws each key in its own row and says them all, a requires on a block by its value', () => {
    const flow = flowOf(
      [
        cell('a', { outputs: ['x', 'y'] }),
        cell('b', {
          when: [
            { on: 'block', block: 'x', value: 'yes' },
            { on: 'block', block: 'y', value: 'no' },
          ],
          requires: [{ on: 'block', block: 'deps', value: 'merged' }],
        }),
      ],
      {
        x: { values: ['yes', 'no'] },
        y: { values: ['yes', 'no'] },
        deps: { values: ['merged', 'open'], outside: 'items' },
      },
    );
    const page = draw({ state: 'compiled', flow });
    expect(texts(page, 'tbody.band .condition-row .block-name')).toEqual(['x', 'y', 'deps']);
    expect(texts(bandRow(page, 'condition-row', 'x'), '.spot .visually-hidden')).toEqual(['b when x = yes']);
    expect(texts(bandRow(page, 'condition-row', 'y'), '.spot .visually-hidden')).toEqual(['b when y = no']);
    expect(texts(bandRow(page, 'condition-row', 'deps'), '.spot .visually-hidden')).toEqual([
      'b requires deps = merged',
    ]);
    expect(bandRow(page, 'condition-row', 'deps').querySelectorAll('.part.precondition')).toHaveLength(1);
    expect(headNames(page)[1]).toBe('b b, engineer, when x = yes, y = no, requires deps = merged');
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    expect(bandRow(page, 'condition-row', 'deps').querySelector('.words')?.textContent).toBe('requires deps = merged');
    const panel = page.querySelector('aside[aria-label="Cell b"]') as HTMLElement;
    expect(texts(panel, '.status-line')).toEqual(['when x = yes, y = no', 'requires deps = merged']);
  });
});

describe('what an agent is sent', () => {
  const text = (t: string): SentPiece => ({ piece: 'text', text: t });
  const part = (fields: Partial<SentPart> & Pick<SentPart, 'what' | 'carried' | 'name' | 'source'>): SentPart => ({
    only: null,
    block: null,
    kind: null,
    pieces: [],
    held: [],
    whole: [],
    ...fields,
  });
  /// The form of a return: a kind of it that asks for the engine's tool `query`, beside a field of its own.
  const FORM = JSON.stringify({
    type: 'object',
    properties: {
      ask: {
        type: 'object',
        properties: { query: { type: 'string' }, note: { type: 'string' } },
        required: ['query'],
      },
    },
  });
  const given = (): Given => ({
    state: 'compiled',
    cells: {
      plan: [
        part({
          what: 'program',
          carried: 'program',
          name: 'program',
          source: 'agents/program.toml',
          pieces: [text('You run in a program.')],
        }),
        part({ what: 'form', carried: 'call', name: 'form', source: 'agents/return.toml', pieces: [text(FORM)] }),
        part({
          what: 'system',
          carried: 'system',
          name: 'general',
          source: 'rules/general.md',
          pieces: [text('Be exact.')],
        }),
        part({
          what: 'input',
          carried: 'message',
          name: 'request',
          source: 'blocks/request.toml',
          block: 'request',
          kind: 'latest',
          pieces: [text('The request:\n'), { piece: 'slot', name: 'request' }],
          held: [text('The request is as before.')],
        }),
        part({
          what: 'task',
          carried: 'message',
          name: 'task',
          source: 'cells/plan.toml',
          pieces: [text('Write the plan.')],
        }),
      ],
    },
    tools: ['query'],
    acts: {
      check: {
        source: 'acts/check.toml',
        words: [['passed', [text('The check passed on '), { piece: 'slot', name: 'head' }]]],
      },
    },
    toolFiles: {
      query: { source: 'agents/tools/query.toml', words: [['what', [text('Reads what the engine holds.')]]] },
    },
    unknown: [],
  });
  /// A part of a panel, by its label.
  const partOf = (panel: HTMLElement, label: string) =>
    [...panel.querySelectorAll('.panel-part')].find((p) =>
      p.querySelector('.panel-part-label')?.textContent?.startsWith(label),
    ) as HTMLElement;

  it('shows an agent’s cell’s parts by the way each reaches it, each with its key, attributes and file', () => {
    const page = draw({ state: 'compiled', flow: small() }, given());
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(texts(panel, '.panel-part-label')).toEqual(['triggered_by 2', 'program 2', 'System prompt 1', 'Message 2']);
    // What runs it, by the triggers its file writes.
    expect(items(partOf(panel, 'triggered_by'))).toEqual(['on_request block request', 'on_opinions block opinions']);
    expect(items(partOf(panel, 'System prompt'))).toEqual([
      'line engineer cells plan, design, answer',
      '3 general rules/general.md 9 characters Show the text',
    ]);
    // The content of a block is shown as sent, with each place filled in a run by its name, and what is sent where the
    // session holds it already; the task is folded.
    expect(items(partOf(panel, 'Message'))).toEqual([
      '4 request kind latest blocks/request.toml The request: {request} held The request is as before.',
      '5 task cells/plan.toml 15 characters Show the text',
    ]);
  });

  it('says the file a task is written in beside the task', () => {
    const flow = small();
    flow.cells[0].taskFile = 'tasks/plan.md';
    const page = draw({ state: 'compiled', flow }, given());
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(items(partOf(panel, 'Message'))[1]).toBe(
      '5 task file tasks/plan.md cells/plan.toml 15 characters Show the text',
    );
  });

  it('draws the form of a return as a tree, the engine’s tools apart from the fields', () => {
    const page = draw({ state: 'compiled', flow: small() }, given());
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    const form = [...partOf(panel, 'program').querySelectorAll('.uses li')].find(
      (li) => li.children[1]?.textContent === 'form',
    ) as HTMLElement;
    expect(form.querySelector('.form-tree')).toBeNull();
    click([...form.querySelectorAll('button')].find((b) => b.textContent === 'Show the text'));
    expect(texts(form, '.form-tree .tree-kind')).toEqual(['ask']);
    expect(texts(form, '.form-tree .tree-tool')).toEqual(['query']);
    expect(texts(form, '.form-tree .tree-required')).toEqual(['*']);
  });

  it('shows what an engine’s cell’s act writes, each text by its key as its file writes it', () => {
    const page = draw({ state: 'compiled', flow: small() }, given());
    click(page.querySelectorAll('thead .cell-head .head-button')[4]);
    const panel = page.querySelector('aside[aria-label="Cell check"]') as HTMLElement;
    const act = partOf(panel, 'act');
    expect(texts(act, 'p .mono')).toEqual(['check', 'acts/check.toml']);
    expect(items(act)).toEqual(['passed The check passed on {head}']);
  });

  it('shows what the engine writes of a block it writes, each text by its key', () => {
    const flow = small();
    (flow.blocks.find((b) => b.name === 'main') as Block).writes = [
      [
        'text',
        [text('The main branch '), { piece: 'slot', name: 'main' }, text(' is at '), { piece: 'slot', name: 'head' }],
      ],
    ];
    const page = draw({ state: 'compiled', flow }, given());
    click(rowOf(page, 'main').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block main"]') as HTMLElement;
    expect(texts(panel, '.panel-part-label')).toEqual(['writes 1']);
    expect(items(partOf(panel, 'writes'))).toEqual(['text The main branch {main} is at {head}']);
  });

  it('opens a tool named in the view’s line with its file and its keys as written', () => {
    const page = draw({ state: 'compiled', flow: flowOf([cell('a', { tools: ['query'] })], {}) }, given());
    click([...page.querySelectorAll('.view-facts button')].find((b) => b.textContent === 'query'));
    const panel = page.querySelector('aside[aria-label="Tool query"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('agents/tools/query.toml');
    expect(items(panel)).toEqual(['what Reads what the engine holds.']);
  });

  it('says why it could not be read, over the chart and in a tool’s panel, and shows the flow without it', () => {
    const why = 'agents/return.toml cannot be read';
    const flow = flowOf([cell('a', { tools: ['query'] })], {});
    const page = draw({ state: 'compiled', flow }, { state: 'unread', why });
    expect(page.querySelector('.read-failed')?.textContent).toContain(why);
    expect(page.querySelector('.matrix')).not.toBeNull();
    click([...page.querySelectorAll('.view-facts button')].find((b) => b.textContent === 'query'));
    const panel = page.querySelector('aside[aria-label="Tool query"]') as HTMLElement;
    expect(panel.querySelector('.read-failed')?.textContent).toContain(why);
  });

  it('is said to have failed with why where its read fails, and the flow is read all the same', async () => {
    const flow = small();
    vi.stubGlobal(
      'fetch',
      vi.fn(async (path: string) =>
        path === '/api/v1/flow'
          ? new Response(JSON.stringify({ state: 'compiled', flow }), { status: 200 })
          : new Response('the flows are not known', { status: 500 }),
      ),
    );
    try {
      const { definition, given } = await load();
      expect(definition.state).toBe('compiled');
      expect(given).toEqual({ state: 'unread', why: '/api/v1/flow/given answered 500: the flows are not known' });
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe('a task written in a file of its own', () => {
  it('is shown as written, with the file it comes from as a key', () => {
    const flow = flowOf([cell('a', { taskFile: 'tasks/better_way.md', outputs: ['x'] })], { x: { shown: true } });
    const page = draw({ state: 'compiled', flow });
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell a"]') as HTMLElement;
    const task = [...panel.querySelectorAll('.panel-part')].find(
      (p) => p.querySelector('.panel-part-label')?.textContent === 'task',
    );
    expect(task?.textContent).toContain('file tasks/better_way.md');
    expect(task?.querySelector('.content')?.textContent).toBe('Do the part of a.');
  });
});

describe('a group’s trigger', () => {
  // r is run by the group planned: a new version of a, which w writes, or of b, which v writes after it.
  const planned = {
    name: 'on_planned',
    what: 'Fires when one of the blocks of planned has a new version.',
    block: 'planned',
    group: ['a', 'b'],
    read: ['a', 'b'],
    do: null,
  };
  const flow = () =>
    flowOf(
      [
        cell('w', { outputs: ['a'] }),
        cell('r', { title: 'Review', runsOn: ['a', 'b'], triggers: [planned] }),
        cell('v', { outputs: ['b'] }),
      ],
      { a: {}, b: {} },
    );

  it('is one row, its group as flow.toml names it, opening the group’s panel', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    const row = rowOf(page, 'on_planned', true);
    expect(row.querySelector('.head-button')?.getAttribute('aria-label')).toBe('Trigger on_planned, group planned');
    expect(row.querySelector('.words')?.textContent).toBe('planned [a, b]');
    expect(texts(row, '.spot .visually-hidden')).toEqual(['w outputs a', 'r triggered_by on_planned', 'v outputs b']);
    expect(row.querySelectorAll('.arrow')).toHaveLength(1);
    click(row.querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Group planned"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('flow.toml');
    expect(texts(panel, '.status-line')).toEqual([
      'on_plannedFires when one of the blocks of planned has a new version.',
    ]);
    expect(items(panel)).toEqual(['a', 'b', 'r Review']);
    expect(row.className).toContain('chosen');
  });

  it('is said in a cell’s panel by its group, and a way back through each of the group’s blocks', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    const panel = page.querySelector('aside[aria-label="Cell r"]') as HTMLElement;
    const triggered = [...panel.querySelectorAll('.panel-part')].find((p) =>
      p.querySelector('.panel-part-label')?.textContent?.startsWith('triggered_by'),
    ) as HTMLElement;
    expect(items(triggered)).toEqual(['1 on_planned group planned way_back [v] read [a, b]']);
    click(rowOf(page, 'b').querySelector('.head-button'));
    const block = page.querySelector('aside[aria-label="Block b"]') as HTMLElement;
    expect(items(block)).toEqual(['v → r triggered_by [on_planned]']);
  });
});

describe('a definition that does not compile', () => {
  it('shows its faults, each with its file and field, in place of the chart', () => {
    const page = draw({
      state: 'faults',
      faults: [{ file: 'plan.toml', field: 'inputs[2]', what: { kind: 'noSuchBlock', block: 'ghost' } }],
    });
    expect(page.querySelector('.matrix')).toBeNull();
    expect(texts(page, '.faults tbody td')).toEqual(['plan.toml', 'inputs[2]', 'The block ghost is not declared in flow.toml.']);
  });
});

describe('the page round the view', () => {
  it('shows the views in the bar, the one shown marked, and switches light and dark, following the system at first', () => {
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    act(() =>
      root.render(
        <Shell route={{ view: 'work', work: 3 }}>
          <p>view</p>
        </Shell>,
      ),
    );
    unmount = () => act(() => root.unmount());
    expect(texts(container, '.bar-tab')).toEqual(['Console', 'Works', 'Flow', 'Observations', 'System', 'Settings']);
    expect(texts(container, '.bar-tab[aria-current="page"]')).toEqual(['Works']);
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
    click([...container.querySelectorAll('.switch-choice')].find((b) => b.textContent === 'Dark'));
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    click([...container.querySelectorAll('.switch-choice')].find((b) => b.textContent === 'Auto'));
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('folds the panel away by its handle, brings it back, and lets go of what is chosen with Escape', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(rowOf(page, 'plan').querySelector('.head-button'));
    const drawer = () => page.querySelector('aside[aria-label="Block plan"]') as HTMLElement | null;
    click(drawer()?.querySelector('.drawer-tab'));
    expect(drawer()?.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(true);
    click(drawer()?.querySelector('.drawer-tab'));
    expect(drawer()?.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(false);
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });
    expect(drawer()).toBeNull();
  });
});

describe('the panel and the widths', () => {
  afterEach(() => localStorage.clear());

  it('has its handle from the start, folded, and opened with nothing chosen says what to choose', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const drawer = page.querySelector('aside[aria-label="Nothing chosen"]') as HTMLElement;
    expect(drawer.querySelector('.drawer-tab')).not.toBeNull();
    expect(drawer.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(true);
    expect(page.querySelector('.view-body')?.className).toContain('panel-folded');
    click(drawer.querySelector('.drawer-tab'));
    expect(drawer.textContent).toContain("Choose a cell's head or a row's name");
    expect(page.querySelector('.view-body')?.className).not.toContain('panel-folded');
  });

  it('names the flow’s tools once, in the view’s line after the limits, each opening its panel', () => {
    const page = draw({ state: 'compiled', flow: flowOf([cell('a', { tools: ['query', 'run_tests'] })], {}) });
    expect(page.querySelector('.view-facts')?.textContent).toBe(
      'limits runs 3, returns 2, calls 40 · tools query, run_tests',
    );
    expect(texts(page, 'tbody .row-label')).not.toContain('tool');
    click([...page.querySelectorAll('.view-facts button')].find((b) => b.textContent === 'query'));
    expect(page.querySelector('aside[aria-label="Tool query"]')).not.toBeNull();
    // A cell of a flow that names the tools of every agent's cell names none of its own.
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell a"]') as HTMLElement;
    expect(texts(panel, '.panel-part-label').some((label) => label.startsWith('tools'))).toBe(false);
  });

  it('says the flow’s limits once, in the view’s line, and a cell’s own limit in its panel, by their keys', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(page.querySelector('.view-facts')?.textContent).toBe('limits runs 3, returns 2, calls 40');
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const plan = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(texts(plan, '.status-line').some((line) => line.startsWith('limits'))).toBe(false);
    click(page.querySelectorAll('thead .cell-head .head-button')[3]);
    const answer = page.querySelector('aside[aria-label="Cell answer"]') as HTMLElement;
    expect(texts(answer, '.status-line')).toContain('limits runs 5');
  });

  it('labels the corner for both: the cells to its right, the blocks under it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, '.corner .label')).toEqual(['Blocks ↓', 'Cells →']);
  });

  it('has a row for each line at the top, its name in the accent, a dot under each cell on it, joined along it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'tbody.band .line-row .block-name.line-name')).toEqual(['engineer', 'review']);
    const engineer = bandRow(page, 'line-row', 'engineer');
    expect(texts(engineer, '.spot .visually-hidden')).toEqual([
      'plan line engineer',
      'design line engineer',
      'answer line engineer',
    ]);
    expect(engineer.querySelectorAll('.part.on-line')).toHaveLength(3);
    // Joined from plan to answer, across judge's column.
    expect(engineer.querySelectorAll('.seg.line')).toHaveLength(6);
    click(engineer.querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Line engineer"]') as HTMLElement;
    expect(panel.querySelector('.drawer-belongs')?.textContent).toBe('lines/engineer.toml');
    expect(texts(panel, '.uses li')).toEqual(['planPlan', 'designDesign', 'answerAnswer comments']);
  });

  it('says a line’s keys as its file writes them, on its row and in its panel', () => {
    const flow = flowOf(
      [cell('a', { line: 'reviewer' }), cell('b', { line: 'engineer' })],
      {},
      'small',
      { reviewer: { levelsAbove: 1, readsOnly: true, rules: ['general', 'engineering'] } },
    );
    const page = draw({ state: 'compiled', flow });
    const reviewer = bandRow(page, 'line-row', 'reviewer');
    expect(reviewer.querySelector('.words')?.textContent).toBe(
      'levels_above 1, reads_only true, rules [general, engineering]',
    );
    // A key the file leaves out is not said.
    expect(bandRow(page, 'line-row', 'engineer').querySelector('.words')?.textContent).toBe('');
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    expect(reviewer.querySelector('.words')?.textContent).toBe('line');
    click(reviewer.querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Line reviewer"]') as HTMLElement;
    expect(texts(panel, '.status-line')).toEqual([
      'levels_above 1',
      'reads_only true',
      'rules [general, engineering]',
    ]);
  });
});
