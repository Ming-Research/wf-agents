// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { levelWords } from '../marks';
import { outlineOf } from '../testing';
import { cellNowWords } from '../works/marks';
import { NO_LEVEL, stateOf, stoppedAt } from '../works/words';
import type { Act, CellNow, Item, Now, Question, Speaking, Wait, WorkPage } from '../works-api';
import type { Block, Flow } from '../api';
import { FLOW, fixtureItems } from './fixtures';
import { ItemPanel, type PageState } from './ItemWindow';

let root: Root | undefined;
let container: HTMLElement;
afterEach(() => {
  if (root !== undefined) act(() => root?.unmount());
  root = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
  vi.restoreAllMocks();
});

const ITEMS = fixtureItems();

/// A block of a cell's own that this source of the engine's writes, the cell writing it too.
const own = (cell: string, name: string, outside: string): Block => ({
  name,
  values: null,
  default: null,
  what: null,
  writtenBy: { cells: [cell], people: false, note: false, outside },
  readBy: { cells: [cell], conditions: [], tests: [], shown: false },
});

/// The made-up flow where the research's cell and the deep research's may ask the owner, as the definition says by
/// questions and words of their own among their outputs, and the review's cells may not.
const ASKING: Flow = {
  ...FLOW,
  cells: FLOW.cells.map((c) =>
    c.id === 'research' || c.id === 'deep_research'
      ? { ...c, outputs: [...c.outputs, `${c.id}_questions`, `${c.id}_words`] }
      : c,
  ),
  blocks: [
    ...FLOW.blocks,
    ...['research', 'deep_research'].flatMap((c) => [own(c, `${c}_questions`, 'answer'), own(c, `${c}_words`, 'words')]),
  ],
};
const itemOf = (n: number) => ITEMS.items.find((i) => i.number === n) as Item;
const pageOf = (i: Item, speaking: Speaking[] = SPEAKING): PageState => ({
  page: {
    summary: i.work,
    title: i.title,
    level: i.level,
    project: i.project,
    history: i.history,
    outline: outlineOf(ASKING),
    flow: ASKING,
    cells: [],
    blocks: [],
    steps: [],
    speaking,
  } satisfies WorkPage,
});

/// The same, with the engineer's flow's own cell ids (R1, D1, I1) in place of the made-up ones, which the discussion
/// writes to by those ids.
const ENGINEER: Record<string, string> = { research: 'R1', deep_research: 'D1', implement: 'I1' };
const engineer = <T,>(x: T): T =>
  JSON.parse(JSON.stringify(x).replace(/"(research|deep_research|implement)"/g, (_, id: string) => `"${ENGINEER[id]}"`)) as T;

/// Where the engine says the owner's words go: what is said to the builder's line reaches the implementation, and
/// no cell of the deep research's line can take any.
const SPEAKING: Speaking[] = [
  { to: { kind: 'line', line: 'builder' }, cell: 'implement', refused: null },
  {
    to: { kind: 'line', line: 'deep_research' },
    cell: null,
    refused: 'no cell of the line deep_research has run yet, and the work stands at none of them',
  },
];

/// What the panel sent, each answered at once, with this where it is given.
type Sent = [number, Act][];
const answered =
  (sent: Sent, answer: unknown = null) =>
  (work: number, what: Act) => {
    sent.push([work, what]);
    return Promise.resolve(answer);
  };

/// Draws the panel of item `n`, or of this item, as the console does, in the same root as the panel drawn before it, if
/// any; `asEngineer` draws item `n` with the engineer's flow's cell ids.
function draw(
  n: number | Item,
  send: (work: number, what: Act) => Promise<unknown>,
  page?: PageState,
  asEngineer = false,
) {
  const item = typeof n !== 'number' ? n : asEngineer ? engineer(itemOf(n)) : itemOf(n);
  if (root === undefined) {
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
  }
  const r = root;
  act(() =>
    r.render(
      <ItemPanel
        item={item}
        page={page === undefined ? (asEngineer ? engineer(pageOf(item)) : pageOf(item)) : page}
        act={send}
        onTitle={() => undefined}
      />,
    ),
  );
  return container;
}

const text = (el: HTMLElement, selector: string) =>
  [...el.querySelectorAll(selector)].map((e) => e.textContent?.replace(/\s+/g, ' ').trim() ?? '');

/// The words of each status line of the window's head, part by part: the item's, then the work's.
const lines = (el: HTMLElement) =>
  [...el.querySelectorAll('.cw-item > .status-line')].map((l) =>
    [...l.children]
      .map((c) => c.textContent?.trim() ?? '')
      .filter((t) => t !== '')
      .join(' '),
  );

/// The work's state line has no row of keys under it: what it says is one line.
const noKeys = (el: HTMLElement) => el.querySelector('.cw-item > .status-line .cw-keys, .cw-item > .status-line .keys') === null;

/// The engine's wait for the owner's answer to this question of the work's, at this version: said by what it asks.
const answering = (name: string, version: number | null = 30): Wait => ({
  key: `answer-${name}`,
  why: `What of ${name}?`,
  ends: { kind: 'answer', question: name, version },
});

const button = (el: HTMLElement, words: string) =>
  [...el.querySelectorAll('button')].find((b) => b.textContent === words) as HTMLButtonElement;

/// Types into a box as the browser would, so that React hears it.
function type(box: HTMLTextAreaElement, words: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(box, words);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

/// The section of the discussion.
const discussion = (el: HTMLElement) =>
  [...el.querySelectorAll<HTMLElement>('.cw-fold')].find((f) =>
    f.querySelector('.cw-fold-head')?.textContent?.includes('Discussion'),
  ) as HTMLElement;

/// Sends the test answers when it likes.
function held() {
  const sent: Sent = [];
  const answers: { yes: () => void; no: (why: string) => void }[] = [];
  const send = (work: number, what: Act) => {
    sent.push([work, what]);
    return new Promise((yes, no) => answers.push({ yes: () => yes(null), no: (why) => no(new Error(why)) }));
  };
  return { sent, answers, send };
}

describe('the panel of the item selected', () => {
  it('heads with the number, the title and the project, and says where the item stands and what its work does', () => {
    const el = draw(12, answered([]));
    expect(text(el, '.drawer-id')).toEqual(['12']);
    expect(text(el, '.drawer-title')).toEqual(['Show the cost of a work']);
    expect(text(el, '.drawer-belongs')).toEqual(['wf-agents']);
    const [itemLine, workLine] = lines(el);
    // The item's line says where it stands, then the level its work's cell runs at, a dash where the engine gives none,
    // then when it was added, and at its right the cell the work stands at, by its id, before the strip (decision
    // a-level-looks-as-the-level-it-runs-at); what it waits for is its work's line's to say.
    expect(itemLine).toBe('Open – Added 1.1d ago review_plan');
    expect(workLine).toContain('Waiting for you review_plan Review the plan');
    expect(noKeys(el)).toBe(true);
    // What it waits for is its card, under the line, in the look of every stop: why, and the button that ends it.
    const card = el.querySelector('[data-wait="limit"]') as HTMLElement;
    expect(card.className).toBe('cw-choice cw-choice-holds');
    expect(text(card, '.cw-choice-q')).toEqual(['The cell ran 3 times in a row for what a cell after it wrote, which is its limit.']);
    // It belongs to no cell, so its frame names none and is unbroken.
    expect(card.querySelector('.cw-choice-by')).toBe(null);
    expect(card.querySelector('button')?.title).toBe('Run the cell again past its limit');
    expect(el.querySelector('.cw-item > .status-line a.link.mono')?.getAttribute('href')).toBe('#/works/12');
  });

  it('says the items a work waits for once, on its line, as the engine gives them', () => {
    const now: Now = { kind: 'waits', at: 'S1', on: [{ kind: 'items', items: [5, 7] }] };
    const el = drawQuestions(now, [], answered([]));
    const [itemLine, workLine] = lines(el);
    expect(itemLine).not.toContain('#5');
    expect(workLine).toContain('Waiting for the items it depends on');
    expect(workLine).toContain('#5 and #7');
    expect(el.textContent?.split('Waiting for the items it depends on').length).toBe(2);
  });

  it("names the item's project by its name alone, and nothing for an item added before items had projects", () => {
    const el = draw(8, answered([]));
    expect(text(el, '.drawer-belongs')).toEqual(['Whitefoot']);
    expect(el.querySelector('.drawer-belongs')?.className).toBe('drawer-belongs mono');
    draw(21, answered([]));
    expect(text(el, '.drawer-title')).toEqual(['Store the versions of each block']);
    expect(el.querySelector('.drawer-belongs')).toBeNull();
  });

  it("says a work the engine paused as waiting for you, its reason on its card, which alone lets it on: the switch shows the owner's own pause", () => {
    const sent: Sent = [];
    const el = draw(4, answered(sent));
    const [, workLine] = lines(el);
    expect(workLine).toContain('Waiting for you');
    expect(noKeys(el)).toBe(true);
    const card = el.querySelector('[data-wait="pause"]') as HTMLElement;
    expect(text(card, '.cw-choice-q')).toEqual([
      "The engine paused the work. The agent's program exited with an error: the session could not be resumed.",
    ]);
    // The cell it is stopped at is filled in the stop's red in its strip, as on its card.
    const marks = [...el.querySelectorAll<HTMLElement>('.cw-item > .status-line .strip .state-mark')];
    expect(marks.filter((m) => m.style.background === 'var(--red)')).toHaveLength(1);
    const pressed = text(el, '.cw-acts [aria-pressed="true"]');
    expect(pressed).toEqual(['Start']);
    act(() => (card.querySelector('button') as HTMLButtonElement).click());
    expect(sent).toEqual([[4, { kind: 'resume' }]]);
    expect(card.querySelector('button')?.textContent).toBe('Resume');
  });

  it("shows the work's cells in the works page's strip, one mark per cell in the flow's order, each by its state", () => {
    for (const n of [12, 6, 4, 9, 2]) {
      const el = draw(n, answered([]));
      const work = itemOf(n).work;
      const cells = work.cells;
      const strip = el.querySelectorAll('.cw-item > .status-line .cw-progress .strip');
      expect(strip).toHaveLength(1);
      const marks = [...strip[0].querySelectorAll<HTMLElement>('.state-mark')];
      expect(marks.map((m) => m.title)).toEqual(cells.map(cellNowWords));
      // The cell the work is stopped at, for the owner or by the engine, filled in red as on its card; no other.
      const stopped = stoppedAt(work);
      expect(marks.filter((m) => m.style.background === 'var(--red)').map((m) => cells[marks.indexOf(m)].cell)).toEqual(
        stopped === null ? [] : [stopped],
      );
      // The level its cell runs at, written on the item's line after its standing, as its card writes it under its
      // number; a dash where the engine gives none.
      const level = el.querySelector('.cw-item > .status-line > .status-word + .mono');
      expect(level?.textContent).toBe(work.level == null ? NO_LEVEL.shown : levelWords(work.level, work.raise ?? 0));
      // The cell it stands at, by its id, just before the strip; none where it stands at none.
      const at = el.querySelector('.cw-item > .status-line .cw-progress .cw-at > .mono');
      expect(at?.textContent ?? null).toBe(stateOf(work).cell);
      // The bright marks pulse, as on the card.
      expect(marks.map((m) => m.classList.contains('pulses'))).toEqual(
        cells.map((c) => c.state === 'running' || c.state === 'calling'),
      );
      // The strip's own words, cell by cell.
      expect(strip[0].getAttribute('aria-label')).toBe(cells.map(cellNowWords).join('; '));
    }
  });

  it('says on the work\'s line what the work waits for and who gives it, and a question a cell asked at the plan\'s end', () => {
    const el = draw(20, answered([]));
    const [, workLine] = lines(el);
    expect(workLine).toContain('Waiting for you implement Implement');
    expect(noKeys(el)).toBe(true);
    const asked = el.querySelector('[data-wait="call-12"]') as HTMLElement;
    expect(text(asked, '.cw-choice-q')).toEqual(['May a page keep its stream open while hidden?']);
    // It holds the work: on the red tint, with no line to say so.
    expect(asked.className).toBe('cw-choice cw-choice-holds');
    // Its frame names the cell it belongs to, broken around the words as a fieldset's legend breaks it: the cell by
    // its id, and the line that cell runs on after it.
    expect(asked.tagName).toBe('FIELDSET');
    expect(text(asked, '.cw-choice-by')).toEqual(['implement builder']);
  });

  it("says a message to the chosen cell's line, names the cell the engine says it reaches, and then the one it gave it to", async () => {
    const sent: Sent = [];
    // The engine gives the words to another cell than the one the page read it would: the page says the engine's.
    const el = draw(12, answered(sent, { reached: 'R1' }), undefined, true);
    // A button for each line that can ask the owner a question, and none for the review's line, which cannot: its line
    // and the cell the engine would give the words to, by its id, or the line alone where it has none.
    expect(text(el, '.cw-to .switch-choice')).toEqual(['builder · I1', 'D1']);
    // A line no cell of which can take the words cannot be picked, and says why on pointing at it (decision
    // only-a-line-that-takes-words-is-picked).
    expect(button(el, 'D1').title).toBe(`No cell takes the words: ${SPEAKING[1].refused}`);
    expect(button(el, 'D1').disabled).toBe(true);
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['builder · I1']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Implement']);
    act(() => button(el, 'builder · I1').click());
    expect(text(el, '.write-readers')).toEqual(['Goes to Implement']);
    expect(button(el, 'Send').disabled).toBe(true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, '  Keep it to one table  ');
    expect(button(el, 'Send').disabled).toBe(false);
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'say', to: { kind: 'line', line: 'builder' }, words: 'Keep it to one table' }]]);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
    // What was said stands as the owner's message, marked as not yet taken by the cell; no line under the box says it.
    expect(text(el, '.cw-done')).toEqual([]);
    const mine = [...el.querySelectorAll('.cw-message-owner')].at(-1);
    expect(mine?.querySelector('.cw-waiting')?.getAttribute('aria-label')).toContain('Not taken yet');
  });

  it('says words to the line of the cell the work waits at for an answer to its question, which the engine says they reach', async () => {
    const sent: Sent = [];
    const i = itemOf(12);
    const waits = engineer({
      ...i,
      work: { ...i.work, now: { kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'one_table' }] } },
    } satisfies Item);
    // The engine says words to the line reach the cell it has reached, the one that waits.
    const speaking: Speaking[] = [
      { to: { kind: 'line', line: 'builder' }, cell: 'research', refused: null },
      SPEAKING[1],
      { to: { kind: 'cell', cell: 'research' }, cell: 'research', refused: null },
    ];
    const el = draw(waits, answered(sent, { reached: 'R1' }), engineer(pageOf(i, speaking)));
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Why two tables?');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'say', to: { kind: 'line', line: 'builder' }, words: 'Why two tables?' }]]);
    // Another line, which the engine says no cell of can take them, cannot be picked.
    act(() => button(el, 'D1').click());
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['builder · R1']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
  });

  it('shows a button for each line that can ask the owner a question and none for one that cannot, and says words to the line of the button chosen', async () => {
    const sent: Sent = [];
    const speaking: Speaking[] = [
      { to: { kind: 'line', line: 'builder' }, cell: 'implement', refused: null },
      { to: { kind: 'line', line: 'deep_research' }, cell: 'deep_research', refused: null },
      { to: { kind: 'line', line: 'judge' }, cell: 'review_code', refused: null },
    ];
    const i = itemOf(12);
    const el = draw(i, answered(sent, { reached: 'deep_research' }), pageOf(i, speaking));
    // Every button where the engine names a cell carries both its line and that cell's id.
    expect(text(el, '.cw-to .switch-choice')).toEqual(['builder · implement', 'deep_research · deep_research']);
    expect([...el.querySelectorAll('.cw-to .switch-choice')].map((b) => b.getAttribute('title'))).toEqual([null, null]);
    act(() => button(el, 'deep_research · deep_research').click());
    expect(text(el, '.write-readers')).toEqual(['Goes to Deep research']);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Look at the second table');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([
      [12, { kind: 'say', to: { kind: 'line', line: 'deep_research' }, words: 'Look at the second table' }],
    ]);
  });

  it('offers, as `@` is typed, the line the message goes to while the work goes on, and names no line of its own', () => {
    const speaking: Speaking[] = [
      { to: { kind: 'line', line: 'builder' }, cell: 'implement', refused: null },
      { to: { kind: 'line', line: 'deep_research' }, cell: 'deep_research', refused: null },
    ];
    const i = itemOf(12);
    const el = draw(i, answered([]), pageOf(i, speaking));
    act(() => button(el, 'deep_research · deep_research').click());
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Look at @');
    expect(text(el, '.cw-names .button')).toEqual(['@deep_research', '@architect · Architect', '@assistant · Assistant']);
  });

  it("draws a section for each datum the flow's card names, in its order and by its name, saying where one has nothing yet", async () => {
    const i = itemOf(12);
    const page = pageOf(i);
    if (page === null || !('page' in page)) throw new Error('no page');
    const flow: Flow = { ...ASKING, card: ['outline', 'request'] };
    const el = draw(i, answered([]), { page: { ...page.page, flow, outline: outlineOf(flow) } });
    await act(async () => undefined);
    expect(text(el, '.cw-body > .cw-fold > .cw-fold-head > span:last-child')).toEqual(['outline', 'request']);
    expect(text(el, '.cw-body > .cw-fold > .cw-fold-body > p.muted')).toEqual([
      'Nothing is written yet',
      'Nothing is written yet',
    ]);
  });

  it('takes a question a cell asked to the discussion, quoted, about that question, on the line of the cell that asked it', async () => {
    const sent: Sent = [];
    const speaking: Speaking[] = [
      { to: { kind: 'line', line: 'builder' }, cell: 'research', refused: null },
      { to: { kind: 'cell', cell: 'research' }, cell: 'research', refused: null },
    ];
    // The work waits at the research for the owner's answer to its question, which its card at the plan's end shows.
    const stale = question('stale-runs', {
      waits: true,
      ask: 'Should a stale run be abandoned at once or kept a day?',
      options: [{ does: 'Abandon it at once', follows: 'Its round is stopped and its cell runs again' }],
    });
    const page = pageOf(itemOf(1), speaking);
    if (page === null || !('page' in page)) throw new Error('no page');
    const asking = engineer({ page: { ...page.page, questions: [stale] } });
    const el = draw(1, answered(sent, { reached: 'R1' }), asking, true);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    await act(async () => button(el, 'Discuss').click());
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['builder · R1']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
    // The discussion's box, not the one the question is answered in.
    type(discussion(el).querySelector('textarea') as HTMLTextAreaElement, 'Kept a day: say why not.');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([
      [
        1,
        {
          kind: 'say',
          to: { kind: 'line', line: 'builder' },
          words: '> Should a stale run be abandoned at once or kept a day?\n> Abandon it at once\n\nKept a day: say why not.',
        },
      ],
    ]);
  });

  it('says why no cell can take a message, and sends it to no cell', async () => {
    const sent: Sent = [];
    const refused = 'no cell of the line builder has run yet, and the work stands at none of them';
    const speaking: Speaking[] = [{ to: { kind: 'line', line: 'builder' }, cell: null, refused }];
    const el = draw(12, answered(sent), engineer(pageOf(itemOf(12), speaking)), true);
    // Where no line takes the words, none is picked, no box is drawn, and why is said once, under the lines.
    // Each reason once: the engine's of the builder's line, and that it says nothing of the deep research's.
    expect(text(el, '.cw-compose > p.muted')).toEqual([
      `No cell takes the words: ${refused}; the engine does not say which cell the words would reach`,
    ]);
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual([]);
    expect(discussion(el).querySelector('textarea')).toBeNull();
    expect(button(el, 'Send')).toBeUndefined();
    expect(sent).toEqual([]);
  });

  it('keeps what is typed for a block of a work, so that it stands when the panel shows it again', () => {
    const el = draw(12, answered([]), undefined, true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Half a thought');
    draw(2, answered([]), undefined, true);
    draw(12, answered([]), undefined, true);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('Half a thought');
  });

  it('pauses a work that goes on, starts one that is paused, and cancels one once the owner confirms and says why', async () => {
    const sent: Sent = [];
    const el = draw(2, answered(sent));
    await act(async () => button(el, 'Pause').click());
    expect(sent).toEqual([[2, { kind: 'pause' }]]);

    draw(3, answered(sent));
    // Pressing the choice already pressed sends nothing.
    await act(async () => button(el, 'Pause').click());
    await act(async () => button(el, 'Start').click());
    // Cancel asks first, with the page's own question: once let go, and once confirmed.
    await act(async () => button(el, 'Cancel').click());
    expect(text(el, 'dialog .confirm-title')).toEqual(['Cancel this work?']);
    await act(async () => (el.querySelector('dialog .confirm-buttons button') as HTMLButtonElement).click());
    expect(el.querySelector('dialog')).toBeNull();
    await act(async () => button(el, 'Cancel').click());
    // A cancel says why: until it does, it is not made.
    expect(button(el, 'Cancel the work').disabled).toBe(true);
    type(el.querySelector('dialog textarea') as HTMLTextAreaElement, ' Joined into #3 ');
    await act(async () => button(el, 'Cancel the work').click());
    expect(sent.slice(1)).toEqual([
      [3, { kind: 'resume' }],
      [3, { kind: 'cancel', why: 'Joined into #3' }],
    ]);
  });

  it('sends what the owner does once: its buttons wait while it is under way', async () => {
    const { sent, answers, send } = held();
    const el = draw(2, send);
    act(() => button(el, 'Pause').click());
    act(() => button(el, 'Pause').click());
    expect(sent).toEqual([[2, { kind: 'pause' }]]);
    expect(button(el, 'Cancel').disabled).toBe(true);
    await act(async () => answers[0].yes());
    expect(button(el, 'Cancel').disabled).toBe(false);
  });

  it('sends a message once: Send waits while it is under way', async () => {
    const { sent, answers, send } = held();
    const el = draw(12, send, undefined, true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Once');
    act(() => button(el, 'Send').click());
    act(() => button(el, 'Send').click());
    expect(sent).toHaveLength(1);
    expect(button(el, 'Send').disabled).toBe(true);
    await act(async () => answers[0].yes());
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
  });

  it('says why what the owner did was refused, under the item it was for and only there', async () => {
    const { answers, send } = held();
    const el = draw(2, send);
    act(() => button(el, 'Pause').click());
    await act(async () => answers[0].no('The work has ended'));
    expect(text(el, '.cw-acts .failed')).toEqual(['The work has ended']);
    expect(button(el, 'Cancel').disabled).toBe(false);
    // Another item: nothing refused there.
    draw(5, send);
    expect(text(el, '.failed')).toEqual([]);
    // A message refused, then another item.
    draw(12, send, undefined, true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'No');
    act(() => button(el, 'Send').click());
    await act(async () => answers[1].no('People do not write it'));
    expect(text(el, '.failed')).toEqual(['People do not write it']);
    draw(20, send);
    expect(text(el, '.failed')).toEqual([]);
  });

  it('offers nothing to do to a work that has ended', () => {
    const el = draw(18, answered([]));
    expect(el.querySelector('.cw-acts')).toBeNull();
    expect(lines(el)[1]).toContain('Merged');
  });

  it('says in its second tab what of the work the lines do not say', () => {
    const el = draw(2, answered([]));
    act(() => button(el, 'Work').click());
    expect(text(el, '.pairs dt')).toEqual(['Rounds', 'Cost', 'Started', 'Sent back', 'Flow']);
    // Started at its first round.
    expect(text(el, '.pairs dd')).toEqual(['61', '$7.31', '1.1d ago', '0', 'building']);
  });

  it('names the cell its work waits at as a cell is shown, its id and its title, and leaves the question to the plan', async () => {
    const el = drawAsking({ kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'scope' }] }, ['scope'], [
      question('scope', { waits: true }),
    ]);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    const [, workLine] = lines(el);
    expect(workLine).toContain('Waiting for you research Initial research W25');
    expect(workLine).not.toContain('scope');
    expect(text(el, '.cw-item > .status-line .cell-id')).toEqual(['research']);
    expect(text(el, '.cw-item > .status-line .cell-title')).toEqual(['Initial research']);
  });

  it('waits for the work to be read before it offers the blocks to write in, and says so when it could not be', () => {
    const el = draw(12, answered([]), null);
    expect(el.querySelector('textarea')).toBeNull();
    expect(text(discussion(el), 'p')).toEqual(["Reading the work's flow"]);
    draw(12, answered([]), { failed: '/api/v1/works/12 answered 500' });
    expect(text(discussion(el), 'p')).toEqual(['The page could not read the work: /api/v1/works/12 answered 500']);
  });
});

/// A question put to the owner, as the work's page gives it, asked by the research.
const question = (name: string, o: Partial<Question> = {}): Question => ({
  name,
  cell: 'research',
  version: 30,
  engine: false,
  waits: false,
  tree: false,
  withdrawn: null,
  ask: `What of ${name}?`,
  options: [
    { does: 'Keep it', follows: 'Nothing changes' },
    { does: 'Drop it', follows: 'The plan is shorter' },
  ],
  recommends: 1,
  answer: null,
  ...o,
});

/// Draws the panel of item 25, its work doing `now` and waiting on the owner for the answers to the questions
/// `holding`, as the engine's list says, its page giving `questions`; paused by the owner where `paused`.
function drawAsking(
  now: Now,
  holding: string[],
  questions: Question[],
  send = answered([]),
  cells?: CellNow[],
  paused = false,
) {
  const i = itemOf(25);
  const versions = new Map(questions.map((q) => [q.name, q.version]));
  const waits = holding.map((name) => answering(name, versions.get(name)));
  const work = { ...i.work, now, waits, cells: cells ?? i.work.cells, paused: paused ? ('owner' as const) : null };
  const item: Item = { ...i, work };
  const page = pageOf(item);
  if (page === null || !('page' in page)) throw new Error('no page');
  container = document.createElement('div');
  document.body.append(container);
  root = createRoot(container);
  const r = root;
  act(() =>
    r.render(
      <ItemPanel
        item={item}
        page={{ page: { ...page.page, questions } }}
        act={send}
        onTitle={() => undefined}
      />,
    ),
  );
  return container;
}

describe("the work's state line, one part for every state", () => {
  const MIN = 60_000;
  /// Each cell of the fixture's flow, at rest but this one, which stands so.
  const cellsAt = (cell: string, state: CellNow['state']): CellNow[] =>
    FLOW.cells.map((c) => ({
      cell: c.id,
      state: c.id === cell ? state : 'done',
      finished: 1,
      record: null,
      open: c.id === cell ? 9 : null,
    }));
  const since = Date.now() - 13 * MIN;
  const at = (now: Now, cells: CellNow[], paused = false) => drawAsking(now, [], [], answered([]), cells, paused);

  it('names the cell as a cell is shown everywhere, its id and its title, an engine cell as an agent cell, whatever the state', () => {
    const states: [Now, CellNow[], string][] = [
      [{ kind: 'round', cell: 'implement', run: 9, round: 2, since, at: null }, cellsAt('implement', 'running'), 'implement'],
      [{ kind: 'round', cell: 'merge', run: 9, round: 1, since, at: null }, cellsAt('merge', 'calling'), 'merge'],
      [
        { kind: 'waits', at: 'check', on: [{ kind: 'turn', holder: 7, since }] },
        cellsAt('check', 'resting'),
        'check',
      ],
    ];
    for (const [now, cells, cell] of states) {
      const el = at(now, cells);
      expect(text(el, '.cw-item > .status-line .cell-id')).toEqual([cell]);
      expect(text(el, '.cw-item > .status-line .cell-title')).toEqual([FLOW.cells.find((c) => c.id === cell)?.title]);
      act(() => root?.unmount());
      root = undefined;
    }
  });

  it("says a round under way as running, in one line, and the work's own pause on the switch alone", () => {
    const el = at({ kind: 'round', cell: 'implement', run: 9, round: 2, since, at: null }, cellsAt('implement', 'running'), true);
    expect(lines(el)[1]).toBe('Running implement Implement W25');
    expect(noKeys(el)).toBe(true);
    expect(text(el, '.cw-acts [aria-pressed="true"]')).toEqual(['Pause']);
  });

  it("says a call of a tool its agent asked for as waiting for that tool where it runs and how long, in the yellow of a call, and no raw input", () => {
    const link = 'https://github.example/actions/runs/316';
    const el = at(
      {
        kind: 'waits',
        at: 'implement',
        on: [
          {
            kind: 'call',
            call: 8,
            who: null,
            what: { kind: 'tool', name: 'run_tests', input: '{"tests": [], "checks": ["lint"]}' },
            made: since,
            at: link,
          },
        ],
      },
      cellsAt('implement', 'calling'),
    );
    // It waits for the tool, neither running nor done, in the yellow of a call (decision a-call-waited-for-is-no-run),
    // linked to the run of CI it runs as, with how long it has waited, then the cell.
    expect(el.querySelector('.cw-item > .status-line:nth-of-type(2) .dot')?.className).toBe('dot dot-calling');
    expect(lines(el)[1]).toMatch(/^Waiting for run_tests in CI 13m \d+s implement Implement /);
    expect(el.querySelector('.cw-item > .status-line:nth-of-type(2) .status-word a')?.getAttribute('href')).toBe(link);
    expect(noKeys(el)).toBe(true);
    expect(el.querySelector('.cw-item')?.textContent).not.toContain('{"tests"');
    // The cell's mark in the strip pulses in the yellow of a call.
    const marks = [...el.querySelectorAll<HTMLElement>('.cw-item > .status-line .strip .state-mark.pulses')];
    expect(marks.map((m) => m.style.background)).toEqual(['var(--yellow)']);
  });

  it("says an engine cell's act under way as running", () => {
    const link = 'https://github.example/actions/runs/250';
    const el = at({ kind: 'round', cell: 'check', run: 9, round: 1, since, at: link }, cellsAt('check', 'calling'));
    expect(lines(el)[1]).toContain('Running check Check with the latest main');
    expect(noKeys(el)).toBe(true);
  });

  it('says an act that waits for the turn as waiting for the work that holds it, in the line', () => {
    const el = at({ kind: 'waits', at: 'check', on: [{ kind: 'turn', holder: 7, since }] }, cellsAt('check', 'resting'));
    expect(el.querySelector('.cw-item > .status-line:nth-of-type(2) .dot')?.className).toBe('dot');
    expect(lines(el)[1]).toContain('Waiting for W7 check Check with the latest main');
    expect(noKeys(el)).toBe(true);
  });

  it('names the cell a paused or held work would go on at, as the engine names it, and none for a faulted work', () => {
    const states: [Now, CellNow[], string | null][] = [
      [{ kind: 'paused', at: 'implement', by: 'owner', why: null }, cellsAt('implement', 'resting'), 'implement'],
      [{ kind: 'paused', at: 'check', by: 'engine', why: 'It failed.' }, cellsAt('check', 'failed'), 'check'],
      [{ kind: 'everything', at: 'implement', by: 'engine' }, cellsAt('implement', 'resting'), 'implement'],
      [{ kind: 'fault', words: 'Two entries at 3' }, cellsAt('implement', 'resting'), null],
    ];
    for (const [now, cells, cell] of states) {
      const el = at(now, cells);
      expect(el.querySelector('.cw-item > .status-line .cell-id')?.textContent ?? null).toBe(cell);
      act(() => root?.unmount());
      root = undefined;
    }
  });

  it('names no cell once the work has ended', () => {
    const el = at({ kind: 'merged' }, cellsAt('merge', 'done'));
    expect(lines(el)[1]).toContain('Merged W25');
    expect(el.querySelector('.cw-item > .status-line .cell-id')).toBeNull();
  });
});

/// The options of the questions drawn, by question, each as its label and whether it is picked.
const options = (el: HTMLElement) =>
  [...el.querySelectorAll('.cw-choice')].map((c) =>
    [...c.querySelectorAll('.cw-option, .cw-settled')].map(
      (o) =>
        `${o.querySelector('.cw-option-label')?.textContent ?? ''}${o.classList.contains('on') || o.classList.contains('cw-settled-taken') ? ' *' : ''}`,
    ),
  );

/// Each line of the diffs in the questions' words, its pieces kept as they are, removed as `-[…]`, added as `+[…]`.
const diffLines = (root: ParentNode) =>
  [...root.querySelectorAll('.cw-choice-q .cw-diff-text')].map((line) =>
    [...line.children]
      .map((p) => (p.tagName === 'DEL' ? `-[${p.textContent}]` : p.tagName === 'INS' ? `+[${p.textContent}]` : p.textContent))
      .join(''),
  );

/// Opens each diff in the questions' words, which is drawn folded away under a row until the owner opens it.
const openDiffs = (root: ParentNode) => {
  for (const row of root.querySelectorAll('.cw-choice-q .cw-file')) act(() => (row as HTMLButtonElement).click());
};

describe('the questions put to the owner, in the panel of the item selected', () => {
  it('shows a question the work waits for at its cell where it arises, with its options, and sends the answer picked', async () => {
    const sent: Sent = [];
    const el = drawAsking(
      { kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'scope' }] },
      ['scope'],
      [question('scope', { waits: true })],
      answered(sent),
    );
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    expect(text(el, '.cw-choice-q')).toEqual(['What of scope?']);
    // It holds the work: on the red tint, edged in red, with no line to say so.
    expect(el.querySelector('.cw-choice')?.className).toBe('cw-choice cw-choice-holds');
    expect(options(el)).toEqual([['Keep itRecommended', 'Drop it']]);
    expect(text(el, '.cw-option-detail')).toEqual(['Nothing changes', 'The plan is shorter']);
    expect(button(el, 'Confirm (0/1)').disabled).toBe(true);
    act(() => (el.querySelectorAll('.cw-option')[1] as HTMLButtonElement).click());
    expect(options(el)).toEqual([['Keep itRecommended', 'Drop it *']]);
    await act(async () => button(el, 'Confirm (1/1)').click());
    expect(sent).toEqual([[25, { kind: 'answer', question: 'scope', version: 30, option: 2 }]]);
    expect(text(el, '.cw-done')).toEqual(['Your answer was sent.']);
  });

  it('shows the questions the merge waits for, each answerable, and an answered one with its answer, confirmed', async () => {
    const sent: Sent = [];
    const merge: Now = {
      kind: 'waits',
      at: 'merge',
      on: [{ kind: 'precondition', test: 'asked = answered', words: 'asked = answered', by: [{ kind: 'owner' }] }],
    };
    const el = drawAsking(
      merge,
      ['measure', 'read-time'],
      [
        question('measure'),
        question('old', { answer: { option: 1, at: 40, why: null } }),
        question('read-time', { cell: 'review_plan', version: 48 }),
        question('gone', { withdrawn: 'Settled in the plan', ask: null, options: [] }),
      ],
      answered(sent),
      // At the merge, every cell before it has run.
      FLOW.cells.map((c) => ({
        cell: c.id,
        state: c.id === 'merge' ? 'notYet' : 'done',
        finished: c.id === 'merge' ? 0 : 1,
        record: null,
        open: null,
      })),
    );
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    const [, workLine] = lines(el);
    expect(workLine).toBe('Waiting for you merge Merge W25');
    expect(text(el, '.cw-choice-q')).toEqual(['What of measure?', 'What of old?', 'What of read-time?']);
    // No line says why the merge waits for them: each card is red and the work's line names the cell.
    expect(el.textContent).not.toContain('The merge waits for this answer');
    // Those the merge waits for hold the work, on the red tint; the one answered stands with its answer, and is not
    // asked again.
    expect([...el.querySelectorAll('.cw-choice')].map((c) => c.className)).toEqual([
      'cw-choice cw-choice-holds',
      'cw-choice cw-choice-done',
      'cw-choice cw-choice-holds',
    ]);
    // A question answered opens folded, down to the first lines of what it asks; the two the merge waits for open
    // whole, each with the button that folds it away.
    const turns = [...el.querySelectorAll('.cw-choice')].map((c) => c.querySelector('.cw-choice-turn')?.textContent);
    expect(turns).toEqual(['Fold it', 'Show it all', 'Fold it']);
    expect(el.querySelectorAll('.cw-choice-few').length).toBe(1);
    expect(options(el)[1]).toEqual([]);
    // Unfolded, it stands with its answer.
    act(() => (el.querySelectorAll('.cw-choice')[1].querySelector('.cw-choice-turn') as HTMLButtonElement).click());
    expect(options(el)[1]).toEqual(['Keep itRecommended *', 'Drop it']);
    expect(el.querySelectorAll('.cw-choice-few').length).toBe(0);
    act(() => (el.querySelectorAll('.cw-choice')[0].querySelectorAll('.cw-option')[0] as HTMLButtonElement).click());
    expect(button(el, 'Confirm (1/2)').disabled).toBe(true);
    act(() => (el.querySelectorAll('.cw-choice')[2].querySelectorAll('.cw-option')[1] as HTMLButtonElement).click());
    await act(async () => button(el, 'Confirm (2/2)').click());
    expect(sent).toEqual([
      [25, { kind: 'answer', question: 'measure', version: 30, option: 1 }],
      [25, { kind: 'answer', question: 'read-time', version: 48, option: 2 }],
    ]);
  });

  it('shows what a question asks as it was written, a change it is about as code', async () => {
    const ask = 'Whether the change may merge.\n\n- `flow/agents/tools/run_tests.toml`: changed\n\n```diff\n-a = 1\n+a = 2\n```';
    const el = drawAsking({ kind: 'waits', at: 'implement', on: [{ kind: 'answer', question: 'protected_files' }] }, [
      'protected_files',
    ], [question('protected_files', { cell: 'implement', engine: true, waits: true, ask })]);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    // A list is drawn as every list is, each item a paragraph; a diff as the text it changes.
    expect(text(el, '.cw-choice-q p')).toEqual(['Whether the change may merge.', 'flow/agents/tools/run_tests.toml: changed']);
    // The diff stands folded under a row that says it is the diff, the words above naming its file.
    expect(text(el, '.cw-choice-q .cw-file')).toEqual(['The diff']);
    expect(diffLines(el)).toEqual([]);
    openDiffs(el);
    expect(diffLines(el)).toEqual(['a = -[1] +[2]']);
  });

  it('shows a question the work has gone on past in the look of a choice of the plan, which stops nothing yet, with no words of its own', async () => {
    const el = drawAsking({ kind: 'next', cell: 'implement' }, [], [question('later')]);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    expect(text(el, '.cw-choice-q')).toEqual(['What of later?']);
    expect(el.querySelector('.cw-choice')?.className).toBe('cw-choice');
    expect(el.textContent).not.toContain('Still to confirm');
    expect(text(el, '.cw-confirm')).toEqual(['Confirm (0/1)']);
  });

  it("draws each option as a choice, apart from the question by a hairline: a radio mark, filled once picked", async () => {
    const el = drawAsking({ kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'scope' }] }, ['scope'], [
      question('scope', { waits: true }),
    ]);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    const card = el.querySelector('[data-wait="answer-scope"]') as HTMLElement;
    const group = card.querySelector('.cw-options') as HTMLElement;
    expect(group.getAttribute('role')).toBe('radiogroup');
    // The options stand after what the question asks, in a part of their own.
    expect(group.previousElementSibling?.className).toBe('cw-choice-q');
    const marks = [...group.querySelectorAll('.cw-option')].map((o) => o.firstElementChild?.className);
    expect(marks).toEqual(['cw-radio', 'cw-radio']);
    const checked = () => [...group.querySelectorAll('.cw-option')].map((o) => o.getAttribute('aria-checked'));
    expect(checked()).toEqual(['false', 'false']);
    act(() => (group.querySelectorAll('.cw-option')[1] as HTMLButtonElement).click());
    expect(checked()).toEqual(['false', 'true']);
    expect(group.querySelectorAll('.cw-option.on')).toHaveLength(1);
  });
});

/// Draws the panel of item 12, its work doing `now` and waiting on the owner for `waits`, as the engine's list says, its
/// page giving `questions`.
function drawQuestions(
  now: Now,
  questions: Question[],
  send: (work: number, what: Act) => Promise<unknown>,
  waits: Wait[] = [],
) {
  const i = itemOf(12);
  const item: Item = { ...i, work: { ...i.work, now, waits } };
  const p = pageOf(item);
  if (p === null || !('page' in p)) throw new Error('no page');
  return draw(item, send, { page: { ...p.page, questions } });
}

/// Picks the option at this place of the question at that place.
const pick = (el: HTMLElement, choice: number, option: number) => {
  const button = el.querySelectorAll('.cw-choice')[choice].querySelectorAll('.cw-option')[option] as HTMLButtonElement;
  act(() => button.click());
};

describe("the questions put to the owner, at the plan's end of the panel", () => {
  it('shows a question the work waits for whole, in its one card, and sends the option picked as the answer to its version', async () => {
    const sent: Sent = [];
    const el = drawQuestions(
      { kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'scope' }] },
      [question('scope', { waits: true })],
      answered(sent),
      [answering('scope')],
    );
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    // One card, which the address of what the work waits for names, and none under the status line.
    expect(el.querySelectorAll('[data-wait="answer-scope"]')).toHaveLength(1);
    expect(el.querySelector('.cw-waits')).toBeNull();
    expect(text(el, '.cw-choice-q')).toEqual(['What of scope?']);
    // It holds the work: on the red tint, edged in red, with no line to say so.
    expect(el.querySelector('.cw-choice')?.className).toBe('cw-choice cw-choice-holds');
    expect(options(el)).toEqual([['Keep itRecommended', 'Drop it']]);
    expect(text(el, '.cw-option-detail')).toEqual(['Nothing changes', 'The plan is shorter']);
    expect(button(el, 'Confirm (0/1)').disabled).toBe(true);
    pick(el, 0, 1);
    expect(options(el)).toEqual([['Keep itRecommended', 'Drop it *']]);
    await act(async () => button(el, 'Confirm (1/1)').click());
    expect(sent).toEqual([[12, { kind: 'answer', question: 'scope', version: 30, option: 2 }]]);
    expect(text(el, '.cw-done')).toEqual(['Your answer was sent.']);
  });

  it('shows a question the merge waits for as holding the work, which waits for you, and sends its answer', async () => {
    const sent: Sent = [];
    const merge: Now = {
      kind: 'waits',
      at: 'merge',
      on: [{ kind: 'precondition', test: 'asked', words: 'asked = answered', by: [{ kind: 'owner' }] }],
    };
    const old = question('old', { answer: { option: 1, at: 40, why: null } });
    const el = drawQuestions(merge, [question('measure'), old], answered(sent), [answering('measure')]);
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    // It waits for you at the merge's check, in one line.
    expect(lines(el)[1]).toContain('Waiting for you merge Merge');
    expect(noKeys(el)).toBe(true);
    // The card the address of the engine's wait for its answer opens, which says what it asks; the one answered asks
    // nothing.
    expect(el.querySelectorAll('[data-wait="answer-measure"]')).toHaveLength(1);
    expect(el.textContent).not.toContain('The merge waits for this answer');
    // The one the merge waits for holds the work, on the red tint; the one answered stands with its answer.
    expect([...el.querySelectorAll('.cw-choice')].map((c) => c.className)).toEqual([
      'cw-choice cw-choice-holds',
      'cw-choice cw-choice-done',
    ]);
    pick(el, 0, 1);
    await act(async () => button(el, 'Confirm (1/1)').click());
    expect(sent).toEqual([[12, { kind: 'answer', question: 'measure', version: 30, option: 2 }]]);
  });

  it('shows a question the work goes on past by the option it recommends, not counted while no merge waits on it, and sends its answer', async () => {
    const sent: Sent = [];
    const later = question('later', { recommends: 2 });
    const el = drawQuestions({ kind: 'next', cell: 'implement' }, [later], answered(sent));
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    expect(el.querySelector('[data-wait]')?.getAttribute('data-wait')).toBe('answer-later');
    expect(text(el, '.cw-choice-q')).toEqual(['What of later?']);
    // It keeps the look of a choice of the plan, with no words of its own.
    expect(el.querySelector('.cw-choice')?.className).toBe('cw-choice');
    expect(el.textContent).not.toContain('Still to confirm');
    expect(options(el)).toEqual([['Keep it', 'Drop itRecommended']]);
    pick(el, 0, 0);
    await act(async () => button(el, 'Confirm (1/1)').click());
    expect(sent).toEqual([[12, { kind: 'answer', question: 'later', version: 30, option: 1 }]]);
  });

  it("shows the engine's question of a protected change with each file, the reason given for it and its change", async () => {
    const sent: Sent = [];
    // As the engine fills its words of the question (`flow/agents/asks.toml`): each file by its path and why, and its
    // diff.
    const ask = [
      'This work changes files the project protects.',
      '`flow/agents/tools/run_tests.toml`: The tool needs a longer timeout.',
      '```diff\n-ms = 1000\n+ms = 2000\n```',
      '`flow/project.toml`: it gives none.',
      '```diff\n+name = "x"\n```',
    ].join('\n\n');
    const protectedFiles = question('protected_files', {
      cell: 'implement',
      engine: true,
      waits: true,
      ask,
      options: [
        { does: 'Let it merge', follows: 'the change merges with the work as it stands.' },
        {
          does: 'Take it back',
          follows:
            'the work takes the change back. It does not merge while it changes these files, unless you let a change to them merge.',
        },
      ],
      recommends: 1,
      version: 44,
    });
    const el = drawQuestions(
      { kind: 'waits', at: 'implement', on: [{ kind: 'answer', question: 'protected_files' }] },
      [protectedFiles],
      answered(sent),
      [answering('protected_files', 44)],
    );
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    const card = el.querySelector('[data-wait="answer-protected_files"]') as HTMLElement;
    expect(card.className).toBe('cw-choice cw-choice-holds');
    expect(text(card, '.cw-choice-q p code')).toEqual(['flow/agents/tools/run_tests.toml', 'flow/project.toml']);
    expect(text(card, '.cw-choice-q p').slice(1)).toEqual([
      'flow/agents/tools/run_tests.toml: The tool needs a longer timeout.',
      'flow/project.toml: it gives none.',
    ]);
    openDiffs(card);
    expect(diffLines(card)).toEqual(['ms = -[1000] +[2000]', '+[name = "x"]']);
    expect(options(el)).toEqual([['Let it mergeRecommended', 'Take it back']]);
    pick(el, 0, 0);
    await act(async () => button(el, 'Confirm (1/1)').click());
    expect(sent).toEqual([[12, { kind: 'answer', question: 'protected_files', version: 44, option: 1 }]]);
  });

  it('shows a question answered with its answer and asks nothing of it, and a question withdrawn not at all', async () => {
    const el = drawQuestions(
      { kind: 'next', cell: 'implement' },
      [
        question('old', { answer: { option: 2, at: 40, why: null } }),
        question('gone', { withdrawn: 'Settled in the plan', ask: null, options: [] }),
      ],
      answered([]),
    );
    // A question is drawn once the texts that may point to it are read.
    await act(async () => undefined);
    expect(text(el, '.cw-choice-q')).toEqual(['What of old?']);
    expect(el.querySelector('.cw-choice')?.className).toBe('cw-choice cw-choice-done');
    // Answered, it opens folded, down to the first lines of what it asks.
    expect(el.querySelector('.cw-choice-turn')?.textContent).toBe('Show it all');
    act(() => (el.querySelector('.cw-choice-turn') as HTMLButtonElement).click());
    // Settled: its options words alone, with no radio mark to press, the one taken marked by a tick that says so.
    expect(options(el)).toEqual([['Keep itRecommended', 'Drop it *']]);
    expect(el.querySelector('.cw-option, .cw-radio')).toBeNull();
    expect(el.querySelector('.cw-settled-taken svg')?.getAttribute('aria-label')).toBe('The option you took');
    expect(el.querySelector('.cw-confirm')).toBeNull();
  });
});
