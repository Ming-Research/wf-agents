// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Comparison, Definition } from './api';
import { FlowPage, Shell } from './App';
import { cell, flowOf, small, test } from './testing';

const NOTHING: Comparison = { state: 'nothingToCompare' };

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// Draws the page for this definition and comparison, and gives its root.
function draw(definition: Definition, comparison: Comparison = NOTHING): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<FlowPage definition={definition} comparison={comparison} />));
  unmount = () => act(() => root.unmount());
  return container;
}

const texts = (root: HTMLElement, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.trim() ?? '');

function click(element: Element | null | undefined) {
  if (!element) throw new Error('nothing to click');
  act(() => (element as HTMLElement).click());
}

/// The change of the task of a cell from `was` to `is`, as the list of what changed shows it.
function taskChange(was: string, is: string): HTMLElement {
  const page = draw(
    { state: 'compiled', flow: flowOf([cell('a', { task: is, outputs: ['x'] })], { x: { shown: true } }) },
    {
      state: 'compared',
      against: { state: 'compiled', flow: flowOf([cell('a', { task: was, outputs: ['x'] })], { x: { shown: true } }) },
      changes: [{ kind: 'task', cell: 'a', before: was, after: is }],
    },
  );
  return page.querySelector('.changes .change') as HTMLElement;
}

/// What the buttons of the cells' heads are called: each cell's id, title, line, and when it runs.
const headNames = (page: HTMLElement) =>
  [...page.querySelectorAll('thead .cell-head .head-button')].map((b) => b.getAttribute('aria-label'));

/// The row of a block.
const rowOf = (page: HTMLElement, block: string) =>
  [...page.querySelectorAll('tbody tr')].find((tr) => tr.querySelector('.block-name')?.textContent === block) as HTMLElement;

describe('the chart', () => {
  it('shows the blocks down the side, the cells across in the order of the walk, and what each cell does with each block', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'thead .cell-head .cell-id')).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(texts(page, 'tbody .block-name')).toEqual([
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
    expect(texts(rowOf(page, 'plan'), '.spot .visually-hidden')).toEqual([
      'plan may write plan',
      'judge reads plan',
      'design reads plan',
      'answer reads plan',
    ]);
    expect(texts(rowOf(page, 'needs_design'), '.spot .visually-hidden')).toEqual([
      'judge may write needs_design',
      'design runs only by the value of needs_design',
    ]);
    expect(rowOf(page, 'needs_design').querySelector('.block-values')?.textContent).toBe('yes|no');
    // A head holds the id and the line alone; the rest is what its button is called.
    expect(texts(page, 'thead .cell-head .head-button')).toEqual([
      'planengineer',
      'judgereview',
      'designengineer',
      'answerengineer',
      'checkengine',
      'mergeengine',
    ]);
    expect(headNames(page)[2]).toBe('design Design, engineer line, runs when needs_design is yes');
    expect(headNames(page)[3]).toBe(
      'answer Answer comments, engineer line, runs when The item has a comment (commented is yes), ' +
        'waits until The owner confirmed the plan (confirmed is yes)',
    );
    // The one way back: judge writes the opinions plan reads, and the arrow is at plan's read.
    expect(page.querySelector('.legend')?.textContent).toContain('way back (1)');
    const arrows = [...page.querySelectorAll('.arrow')];
    expect(arrows).toHaveLength(1);
    expect(arrows[0].closest('td')?.textContent).toBe('plan reads opinions');
  });

  it('marks each block a named test reads, by the condition’s mark or by a mark of a precondition’s own', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(rowOf(page, 'commented'), '.spot .visually-hidden')).toEqual([
      'answer runs only by the value of commented',
    ]);
    expect(texts(rowOf(page, 'confirmed'), '.spot .visually-hidden')).toEqual(['answer waits on the value of confirmed']);
    expect(texts(rowOf(page, 'ci'), '.spot .visually-hidden')).toEqual([
      'check may write ci',
      'merge reads, waits on the value of ci',
    ]);
    expect(rowOf(page, 'confirmed').querySelectorAll('.part.precondition')).toHaveLength(1);
    expect(rowOf(page, 'commented').querySelectorAll('.part.condition')).toHaveLength(1);
    expect(page.querySelector('.legend')?.textContent).toContain('its precondition reads');
  });

  it('says the source outside that writes a block on its row and in its panel', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(rowOf(page, 'main').querySelector('.block-source')?.textContent).toBe('main');
    expect(rowOf(page, 'plan').querySelector('.block-source')).toBeNull();
    click(rowOf(page, 'main').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block main"]') as HTMLElement;
    expect(panel.textContent).toContain('the outside (main)');
  });

  it('shows everything about a cell chosen by its column’s head, and about a block chosen from it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const panel = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(panel.textContent).toContain('Shares its session with design, answer');
    const rows = [...panel.querySelectorAll('.uses li')].map((li) =>
      [...li.children].map((c) => c.textContent?.trim()).join(' '),
    );
    expect(rows.slice(0, 3)).toEqual([
      '1 request written by people',
      '2 opinions written by judge, people way back from judge',
      'plan read by judge, design, answer',
    ]);
    click([...panel.querySelectorAll('button.link')].find((b) => b.textContent === 'opinions'));
    const block = page.querySelector('aside[aria-label="Block opinions"]') as HTMLElement;
    expect(block.textContent).toContain('judge → plan');
  });

  it('shows an agent’s cell’s task with its line breaks, and the act that carries out an engine’s cell', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const heads = page.querySelectorAll('thead .cell-head .head-button');
    click(heads[0]);
    const plan = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(texts(plan, '.panel-part-label')).toEqual(['Reads 2', 'May write 1', 'Task', 'Session']);
    expect(plan.querySelector('.content')?.textContent).toBe('Write the plan from the request.\nSay what it leaves out.');
    click(heads[4]);
    const check = page.querySelector('aside[aria-label="Cell check"]') as HTMLElement;
    expect(texts(check, '.panel-part-label')).toEqual(['Reads 2', 'May write 1', 'Act']);
    const act = [...check.querySelectorAll('.panel-part')].find((p) => p.querySelector('.panel-part-label')?.textContent === 'Act');
    expect(act?.querySelector('.mono')?.textContent).toBe('check');
  });

  it('shows a change of a line break alone in a task by a visible mark, the line breaks kept', () => {
    const was = 'Write the plan.\nSay what it leaves out.';
    const is = 'Write the plan.\n\nSay what it leaves out.';
    const change = taskChange(was, is);
    expect(change.querySelector('.change-text')?.textContent).toBe('Write the plan.\n↵\nSay what it leaves out.');
    expect(texts(change, 'ins')).toEqual(['↵']);
    expect(change.querySelector('del')).toBeNull();
    expect(change.textContent).not.toContain('→');
  });

  it('shows a change of a task as one text, the words taken out and put in marked', () => {
    const was = 'Write the plan from the request.\nSay what it leaves out.\n\nKeep it short.';
    const is = 'Write the plan from the request and its comments.\nSay what it leaves out.\n\nKeep it brief.';
    const change = taskChange(was, is);
    expect(change.querySelectorAll('.change-text')).toHaveLength(1);
    expect(texts(change, 'del')).toEqual(['request.', 'short.']);
    expect(texts(change, 'ins')).toEqual(['request and its comments.', 'brief.']);
    const text = change.querySelector('.change-text') as HTMLElement;
    // What stayed is plain, in the text itself, its line breaks as written.
    const plain = [...text.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE).map((n) => n.textContent);
    expect(plain).toEqual(['Write the plan from the ', '\nSay what it leaves out.\n\nKeep it ']);
  });

  it('shows a task whose words mostly changed as what it was, then what it is, each whole', () => {
    const was = 'Write the plan from the request.';
    const is = 'Review the code and its tests, and say whether you are content.';
    const change = taskChange(was, is);
    expect(change.querySelector('del.change-text')?.textContent).toBe(was);
    expect(change.querySelector('ins.change-text')?.textContent).toBe(is);
  });

  it('marks among what a cell may write the blocks it writes every time, and says a change of them', () => {
    const judge = (mustWrite: string[]) =>
      flowOf([cell('plan', { inputs: ['opinions'] }), cell('judge', { outputs: ['opinions', 'verdict'], mustWrite })], {
        opinions: {},
        verdict: { values: ['yes', 'no'], shown: true },
      });
    const page = draw(
      { state: 'compiled', flow: judge(['verdict']) },
      {
        state: 'compared',
        against: { state: 'compiled', flow: judge([]) },
        changes: [{ kind: 'mustWriteAdded', cell: 'judge', block: 'verdict' }],
      },
    );
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    const panel = page.querySelector('aside[aria-label="Cell judge"]') as HTMLElement;
    const mayWrite = [...panel.querySelectorAll('.panel-part')].find((p) =>
      p.querySelector('.panel-part-label')?.textContent?.startsWith('May write'),
    ) as HTMLElement;
    const rows = [...mayWrite.querySelectorAll('.uses li')].map((li) =>
      [...li.children].map((c) => c.textContent?.trim()).join(' '),
    );
    expect(rows).toEqual(['opinions read by plan way back to plan', 'verdict written every time shown']);
    expect(texts(panel, '.changes-here li')).toEqual(['Writes every timeverdict']);
  });

  it('shows everything about a block chosen by its row’s name', () => {
    const page = draw({ state: 'compiled', flow: small() });
    click(rowOf(page, 'ci').querySelector('.head-button'));
    const panel = page.querySelector('aside[aria-label="Block ci"]') as HTMLElement;
    expect(panel.textContent).toContain('a choice of green, red');
    expect(texts(panel, '.status-line')).toContain('Read bymerge; the test may_merge');
    expect(panel.querySelector('[title="The work may merge (ci is not red)"]')?.textContent).toBe('may_merge');
    expect(rowOf(page, 'ci').className).toContain('chosen');
  });
});

describe('the column of words', () => {
  const flow = () =>
    flowOf(
      [
        cell('a', { title: 'First', outputs: ['x', 'y'], mustWrite: ['x'] }),
        cell('b', {
          title: 'Second',
          condition: { on: 'block', block: 'y', value: 'yes' },
          inputs: ['x', 'said'],
          input: { x: { kind: 'latest', why: 'It is what to judge.' } },
          outputs: ['said'],
        }),
      ],
      { x: { what: 'The plan.' }, y: { values: ['yes', 'no'] }, said: { people: true, what: 'What was said of it.' } },
    );
  /// The words of each row, by its block.
  const words = (page: HTMLElement) =>
    Object.fromEntries(
      [...page.querySelectorAll('tbody tr')].map((tr) => [
        tr.querySelector('.block-name')?.textContent,
        tr.querySelector('.words')?.textContent,
      ]),
    );

  it('says what each block is, and of the cell chosen what it says of each block it has to do with', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    expect(words(page)).toEqual({ x: 'The plan.', y: '', said: 'What was said of it.' });
    expect(page.querySelector('.words-label')?.textContent).toBe('What each block is');
    expect(page.querySelector('.words-about')?.textContent).toBe('');

    const heads = page.querySelectorAll('thead .cell-head .head-button');
    click(heads[1]);
    expect(words(page)).toEqual({
      x: 'takes the latest: It is what to judge.',
      y: 'runs when y is yes',
      said: 'reads; may write, brought up to date: What was said of it.',
    });
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each block');
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer line · runs when y is yes');

    click(heads[0]);
    expect(words(page)).toEqual({ x: 'writes every time: The plan.', y: 'may write', said: 'What was said of it.' });
    expect(rowOf(page, 'said').querySelector('.words')?.className).toContain('beside');
  });

  it('says what a block is in the block’s panel too', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    click(rowOf(page, 'x').querySelector('.head-button'));
    expect(texts(page.querySelector('aside[aria-label="Block x"]') as HTMLElement, '.status-line')).toContain(
      'What it isThe plan.',
    );
    click(rowOf(page, 'y').querySelector('.head-button'));
    expect(page.querySelector('aside[aria-label="Block y"]')?.textContent).not.toContain('What it is');
  });

  it('names over the words the cell under the pointer, and else the cell chosen', () => {
    const page = draw({ state: 'compiled', flow: flow() });
    const heads = page.querySelectorAll('thead .cell-head .head-button');
    const point = (at: Element | null) => act(() => void at?.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })));
    click(heads[1]);
    point(heads[0]);
    expect(page.querySelector('.words-about')?.textContent).toBe('a First · engineer line');
    // The rows still say what the chosen cell says.
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each block');
    point(page.querySelector('.words-head'));
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer line · runs when y is yes');
  });
});

describe('the chart when comparing', () => {
  it('marks over the words what a change changed of the cell named there, and says in a head’s name what became of it', () => {
    const blocks = { y: { values: ['yes', 'no'] } };
    const before = flowOf([cell('a', { outputs: ['y'] }), cell('b', { title: 'Second', inputs: ['y'] })], blocks);
    const when = { on: 'block', block: 'y', value: 'yes' } as const;
    const after = flowOf(
      [cell('a', { outputs: ['y'] }), cell('b', { title: 'Second', inputs: ['y'], condition: when })],
      blocks,
    );
    const page = draw(
      { state: 'compiled', flow: after },
      {
        state: 'compared',
        against: { state: 'compiled', flow: before },
        changes: [{ kind: 'condition', cell: 'b', before: null, after: when }],
      },
    );
    expect(headNames(page)).toEqual(['a a, engineer line', 'b Second, engineer line, runs when y is yes, changed']);
    click(page.querySelectorAll('thead .cell-head .head-button')[1]);
    expect(texts(page, '.words-about .changed-field')).toEqual(['runs when y is yes']);
  });

  it('gives each cell’s column an edge to drag', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(page.querySelectorAll('thead .cell-head .edge')).toHaveLength(6);
  });
});

describe('a comparison', () => {
  it('lists each change under its place and marks it on the chart', () => {
    const before = flowOf([cell('a', { title: 'A', outputs: ['x'] }), cell('b', { title: 'B', inputs: ['x'] })], { x: {} });
    const after = flowOf([cell('a', { title: 'First', outputs: ['x'] }), cell('b', { title: 'B', inputs: ['x', 'y'] })], {
      x: {},
      y: { people: true },
    });
    const page = draw(
      { state: 'compiled', flow: after },
      {
        state: 'compared',
        against: { state: 'compiled', flow: before },
        changes: [
          { kind: 'title', cell: 'a', before: 'A', after: 'First' },
          { kind: 'inputAdded', cell: 'b', block: 'y' },
          { kind: 'blockAdded', block: 'y' },
        ],
      },
    );
    expect(texts(page, '.change-group h3')).toEqual(['a First', 'b B', 'Block y']);
    expect(texts(page, '.changes .change')).toEqual(['TitleA → First', 'Readsy', 'Declared']);
    expect(page.querySelector('thead .cell-head')?.className).toContain('status-changed');
    expect(rowOf(page, 'y').className).toContain('status-added');
    expect(page.querySelectorAll('.part.added')).toHaveLength(1);

    click([...page.querySelectorAll('button')].find((b) => b.textContent === 'As it was'));
    expect(headNames(page)).toEqual(['a A, engineer line, changed', 'b B, engineer line, changed']);
  });
});

describe('a test of what stands', () => {
  it('is said with what it does not count in the head of the cell that waits on it and in its panel', () => {
    const approved = test('approved', 'No judge refuses', 'noneStanding', ['verdict', 'deep_verdict'], 'no');
    const flow = flowOf(
      [
        cell('judge', { outputs: ['verdict', 'deeper'] }),
        cell('deep', { condition: { on: 'block', block: 'deeper', value: 'yes' }, outputs: ['deep_verdict'] }),
        cell('merge', { kind: 'engine', line: null, effort: null, preconditions: [approved] }),
      ],
      { verdict: { values: ['yes', 'no'] }, deeper: { values: ['yes', 'no'] }, deep_verdict: { values: ['yes', 'no'] } },
    );
    const page = draw({ state: 'compiled', flow });
    const words = 'No judge refuses (none of verdict, deep_verdict is no, not counting what a cell skipped now wrote)';
    expect(headNames(page)[2]).toBe(`merge merge, engine, waits until ${words}`);
    click(page.querySelectorAll('thead .cell-head .head-button')[2]);
    expect(page.querySelector('aside[aria-label="Cell merge"]')?.textContent).toContain(words);
  });
});

describe('a comparison of tests and sources', () => {
  it('lists a test changed and a block’s source changed as it lists the others, and marks the source on the chart', () => {
    const was = test('ready', 'Ready', 'all', ['x']);
    const is = test('ready', 'Ready', 'any', ['x', 'y']);
    const blocks = (outside: string) => ({
      x: { people: true, values: ['yes', 'no'] },
      y: { values: ['yes', 'no'], outside },
    });
    const before = flowOf([cell('a', { preconditions: [was] })], blocks('mirror'));
    const after = flowOf([cell('a', { preconditions: [is] })], blocks('main'));
    const page = draw(
      { state: 'compiled', flow: after },
      {
        state: 'compared',
        against: { state: 'compiled', flow: before },
        changes: [
          { kind: 'testChanged', test: 'ready', before: was, after: is },
          { kind: 'outside', block: 'y', before: 'mirror', after: 'main' },
        ],
      },
    );
    expect(texts(page, '.change-group h3')).toEqual(['Test ready', 'Block y']);
    expect(texts(page, '.changes .change')).toEqual([
      'Holds whenx is yes → any of x, y is yes',
      'Written by the outsidemirror → main',
    ]);
    expect(texts(page, '.changes del')).toEqual(['x is yes', 'mirror']);
    expect(texts(page, '.changes ins')).toEqual(['any of x, y is yes', 'main']);
    expect(rowOf(page, 'y').className).toContain('status-changed');
    expect(rowOf(page, 'y').querySelector('.block-source')?.className).toContain('changed-field');
    // The precondition's mark on y is put in by the change.
    expect(rowOf(page, 'y').querySelectorAll('.part.precondition.added')).toHaveLength(1);
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
    expect(drawer.textContent).toContain("Choose a cell's head or a block's name");
    expect(page.querySelector('.view-body')?.className).not.toContain('panel-folded');
  });

  it('opens with the list of changes when comparing', () => {
    const flow = small();
    const page = draw(
      { state: 'compiled', flow },
      {
        state: 'compared',
        against: { state: 'compiled', flow },
        changes: [{ kind: 'title', cell: 'plan', before: 'Plan it', after: 'Plan' }],
      },
    );
    const drawer = page.querySelector('aside[aria-label="What changed"]') as HTMLElement;
    expect(drawer.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(false);
  });

  it('stays folded when comparing finds nothing changed, and says so once opened', () => {
    const flow = small();
    const page = draw(
      { state: 'compiled', flow },
      { state: 'compared', against: { state: 'compiled', flow }, changes: [] },
    );
    const drawer = page.querySelector('aside[aria-label="What changed"]') as HTMLElement;
    expect(drawer.querySelector('.drawer-inner')?.hasAttribute('hidden')).toBe(true);
    click(drawer.querySelector('.drawer-tab'));
    expect(drawer.textContent).toContain('Nothing: the two definitions compile to the same flow');
  });

  it('says the flow’s limits once, in the view’s line, and a cell’s own limit in its panel', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(page.querySelector('.view-facts')?.textContent).toBe(
      'Limits of a cell: 3 ways round in a row, 2 returns sent back, 40 calls',
    );
    click(page.querySelectorAll('thead .cell-head .head-button')[0]);
    const plan = page.querySelector('aside[aria-label="Cell plan"]') as HTMLElement;
    expect(plan.textContent).not.toContain('Limit');
    click(page.querySelectorAll('thead .cell-head .head-button')[3]);
    const answer = page.querySelector('aside[aria-label="Cell answer"]') as HTMLElement;
    expect(texts(answer, '.status-line')).toContain(
      "Limits of its own5 runs in a row that are ways round (the flow's: 3)",
    );
  });

  it('labels the corner for both: the cells to its right, the blocks under it', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, '.corner .label')).toEqual(['Blocks ↓', 'Cells →']);
  });

  it('marks the lines’ names, not the engine', () => {
    const page = draw({ state: 'compiled', flow: small() });
    const lines = [...page.querySelectorAll('thead .cell-line')].map((e) => [
      e.textContent,
      e.classList.contains('line-name'),
    ]);
    expect(lines).toEqual([
      ['engineer', true],
      ['review', true],
      ['engineer', true],
      ['engineer', true],
      ['engine', false],
      ['engine', false],
    ]);
  });
});
