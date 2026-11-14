// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { Change, Definition } from './api';
import { FlowPage, Shell } from './App';
import { ChangeLine } from './Side';
import { cell, flowOf, small, test } from './testing';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

/// Draws the page for this definition, and gives its root.
function draw(definition: Definition): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<FlowPage definition={definition} />));
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

/// Whether a row of the chart stands for a trigger.
const isTrigger = (tr: Element) => tr.querySelector('.block-head .tool-label')?.textContent === 'trigger';

/// The rows of the triggers, or of the data, by their names.
const rows = (page: HTMLElement, triggers: boolean) =>
  [...page.querySelectorAll('tbody tr')].filter((tr) => tr.querySelector('.block-name') && isTrigger(tr) === triggers);

/// The row of a datum, or of a trigger, of this name.
const rowOf = (page: HTMLElement, name: string, trigger = false) =>
  rows(page, trigger).find((tr) => tr.querySelector('.block-name')?.textContent === name) as HTMLElement;

/// The change drawn as the list of what changed draws it.
function changeOf(change: Change): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() => root.render(<ChangeLine change={change} />));
  unmount = () => act(() => root.unmount());
  return container.querySelector('.change') as HTMLElement;
}

describe('the chart', () => {
  it('shows the triggers’ rows over the data’s, the cells across in the order of the walk, and what each cell does with each', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(page, 'thead .cell-head .cell-id')).toEqual(['plan', 'judge', 'design', 'answer', 'check', 'merge']);
    expect(rows(page, true).map((tr) => tr.querySelector('.block-name')?.textContent)).toEqual([
      'request',
      'plan',
      'opinions',
      'design',
      'comments',
      'main',
      'ci',
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
    expect(texts(rowOf(page, 'plan', true), '.spot .visually-hidden')).toEqual([
      'plan outputs plan',
      'judge triggered_by plan',
      'design triggered_by plan',
      'answer triggered_by plan',
    ]);
    expect(texts(rowOf(page, 'plan'), '.spot .visually-hidden')).toEqual([
      'plan outputs plan',
      'judge context plan',
      'design context plan',
      'answer context plan',
    ]);
    expect(texts(rowOf(page, 'needs_design'), '.spot .visually-hidden')).toEqual([
      'judge outputs needs_design',
      'design when needs_design',
    ]);
    expect(rowOf(page, 'needs_design').querySelector('.block-values')?.textContent).toBe('yes|no');
    // A head holds the id and the line alone; the rest is what its button is called, by the cell's keys.
    expect(texts(page, 'thead .cell-head .head-button')).toEqual([
      'planengineer',
      'judgereview',
      'designengineer',
      'answerengineer',
      'checkengine',
      'mergeengine',
    ]);
    expect(headNames(page)[2]).toBe('design Design, engineer, when needs_design is yes');
    expect(headNames(page)[3]).toBe('answer Answer comments, engineer, when has_comment, requires [plan_confirmed]');
    // The one way back: judge writes the opinions whose trigger runs plan, and the arrow is at plan's mark.
    expect(page.querySelector('.legend')?.textContent).toContain('way back (1)');
    const arrows = [...page.querySelectorAll('.arrow')];
    expect(arrows).toHaveLength(1);
    expect(arrows[0].closest('td')?.textContent).toBe('plan triggered_by opinions');
  });

  it('marks on a datum’s row each block a named test reads, by the condition’s mark or by a precondition’s', () => {
    const page = draw({ state: 'compiled', flow: small() });
    expect(texts(rowOf(page, 'commented'), '.spot .visually-hidden')).toEqual(['answer when commented']);
    expect(texts(rowOf(page, 'confirmed'), '.spot .visually-hidden')).toEqual(['answer requires confirmed']);
    expect(texts(rowOf(page, 'ci'), '.spot .visually-hidden')).toEqual(['check outputs ci', 'merge requires ci']);
    expect(texts(rowOf(page, 'ci', true), '.spot .visually-hidden')).toEqual(['check outputs ci', 'merge triggered_by ci']);
    expect(rowOf(page, 'confirmed').querySelectorAll('.part.precondition')).toHaveLength(1);
    expect(rowOf(page, 'commented').querySelectorAll('.part.condition')).toHaveLength(1);
    expect(page.querySelector('.legend')?.textContent).toContain('requires');
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
    expect(items(panel).slice(0, 6)).toEqual([
      '1 request written_by people',
      '2 opinions written_by judge, people way_back [judge]',
      '1 task',
      '2 request kind written_by people',
      '3 opinions kind written_by judge, people',
      'plan read_by cells judge, design, answer',
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
    expect(texts(plan, '.panel-part-label')).toEqual(['triggered_by 2', 'context 3', 'outputs 1', 'tools 0', 'task', 'line']);
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

describe('a change of a task', () => {
  const task = (before: string, after: string): Change => ({ kind: 'task', cell: 'a', before, after });

  it('shows a change of a line break alone by a visible mark, the line breaks kept', () => {
    const change = changeOf(task('Write the plan.\nSay what it leaves out.', 'Write the plan.\n\nSay what it leaves out.'));
    expect(change.querySelector('.change-text')?.textContent).toBe('Write the plan.\n↵\nSay what it leaves out.');
    expect(texts(change, 'ins')).toEqual(['↵']);
    expect(change.querySelector('del')).toBeNull();
    expect(change.textContent).not.toContain('→');
  });

  it('shows it as one text, the words taken out and put in marked', () => {
    const was = 'Write the plan from the request.\nSay what it leaves out.\n\nKeep it short.';
    const is = 'Write the plan from the request and its comments.\nSay what it leaves out.\n\nKeep it brief.';
    const change = changeOf(task(was, is));
    expect(change.querySelectorAll('.change-text')).toHaveLength(1);
    expect(texts(change, 'del')).toEqual(['request.', 'short.']);
    expect(texts(change, 'ins')).toEqual(['request and its comments.', 'brief.']);
    const text = change.querySelector('.change-text') as HTMLElement;
    // What stayed is plain, in the text itself, its line breaks as written.
    const plain = [...text.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE).map((n) => n.textContent);
    expect(plain).toEqual(['Write the plan from the ', '\nSay what it leaves out.\n\nKeep it ']);
  });

  it('shows one whose words mostly changed as what it was, then what it is, each whole', () => {
    const was = 'Write the plan from the request.';
    const is = 'Review the code and its tests, and say whether you are content.';
    const change = changeOf(task(was, is));
    expect(change.querySelector('del.change-text')?.textContent).toBe(was);
    expect(change.querySelector('ins.change-text')?.textContent).toBe(is);
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
    expect(page.querySelector('.words-label')?.textContent).toBe('What each block is');
    expect(page.querySelector('.words-about')?.textContent).toBe('');

    const heads = page.querySelectorAll('thead .cell-head .head-button');
    click(heads[1]);
    expect(words(page)).toEqual({
      x: 'triggered_by x; context latest: It is what to judge.',
      y: 'when y is yes',
      said: 'triggered_by said; context; outputs: What was said of it.',
    });
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each block');
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer · when y is yes');

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
    expect(page.querySelector('.words-label')?.textContent).toBe('What b says of each block');
    point(page.querySelector('.words-head'));
    expect(page.querySelector('.words-about')?.textContent).toBe('b Second · engineer · when y is yes');
  });
});

describe('a test of what stands', () => {
  it('is named by its name in the head of the cell that waits on it and in its panel', () => {
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
    expect(headNames(page)[2]).toBe('merge merge, engine, requires [approved]');
    click(page.querySelectorAll('thead .cell-head .head-button')[2]);
    expect(texts(page.querySelector('aside[aria-label="Cell merge"]') as HTMLElement, '.status-line')).toContain(
      'requires [approved]',
    );
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
