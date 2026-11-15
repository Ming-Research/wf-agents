// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { cellNowWords } from '../works/marks';
import type { Act, Item, Speaking, WorkPage } from '../works-api';
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

/// A block the engine's source of answers writes.
const ofAnswers = (name: string): Block => ({
  name,
  values: null,
  default: null,
  what: null,
  writtenBy: { cells: [], people: false, note: false, outside: 'answer' },
  readBy: { cells: [], conditions: [], tests: [], shown: false },
});

/// The engineer's flow where the research's cell and the deep research's may write the work's questions, as the
/// definition says by the record of the questions and the answers among their outputs, which each one's trigger of an
/// answer reads, and the review's cells may not.
const ASKING: Flow = {
  ...FLOW,
  cells: FLOW.cells.map((c) =>
    c.id === 'research' || c.id === 'deep_research'
      ? {
          ...c,
          outputs: [...c.outputs, 'q_and_a'],
          triggers: [
            ...c.triggers,
            { name: 'on_answer', what: null, block: `${c.id}_answered`, group: [], read: ['q_and_a'], do: null },
          ],
        }
      : c,
  ),
  blocks: [...FLOW.blocks, ofAnswers('q_and_a'), ofAnswers('research_answered'), ofAnswers('deep_research_answered')],
};
const itemOf = (n: number) => ITEMS.items.find((i) => i.number === n) as Item;
const pageOf = (i: Item, speaking: Speaking[] = SPEAKING): PageState => ({
  page: {
    summary: i.work,
    title: i.title,
    level: i.level,
    project: i.project,
    history: i.history,
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

/// Where the engine says the owner's words go: what is said to the engineer's line reaches the implementation, and
/// no cell of the deep research's line can take any.
const SPEAKING: Speaking[] = [
  { to: { kind: 'line', line: 'engineer' }, cell: 'implement', refused: null },
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
        waitsFor={[11]}
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
    expect(text(el, '.drawer-belongs')).toEqual(['project wf-agents']);
    const [itemLine, workLine] = lines(el);
    expect(itemLine).toBe('Open Added 1.1d ago Waits for 11 to merge');
    expect(workLine).toContain(
      'Waiting for you Review the plan reached its limit: 3 runs in a row for what a cell after it wrote W12',
    );
    expect(el.querySelector('.cw-item > .status-line a.link.mono')?.getAttribute('href')).toBe('#/works/12');
  });

  it("names the item's project by its key and value, and nothing for an item added before items had projects", () => {
    const el = draw(8, answered([]));
    expect(text(el, '.drawer-belongs .muted.mono')).toEqual(['project']);
    expect(text(el, '.drawer-belongs .mono:not(.muted)')).toEqual(['Whitefoot']);
    draw(21, answered([]));
    expect(text(el, '.drawer-title')).toEqual(['Store the versions of each block']);
    expect(el.querySelector('.drawer-belongs')).toBeNull();
  });

  it('says a work the engine paused as halted, with the reason, and shows it paused on the switch that starts it', () => {
    const el = draw(4, answered([]));
    const [, workLine] = lines(el);
    expect(workLine).toContain("Halted by the engine The agent's program exited with an error: the session could not be resumed.");
    expect(el.querySelectorAll('.cw-item > .status-line .strip .state-failed')).toHaveLength(1);
    expect(button(el, 'Pause').getAttribute('aria-pressed')).toBe('true');
    expect(button(el, 'Start').getAttribute('aria-pressed')).toBe('false');
  });

  it("shows the work's cells in the works page's strip, one mark per cell in the flow's order, each by its state", () => {
    for (const n of [12, 6, 4, 9, 2]) {
      const el = draw(n, answered([]));
      const cells = itemOf(n).work.cells;
      const strip = el.querySelectorAll('.cw-item > .status-line .cw-progress .strip');
      expect(strip).toHaveLength(1);
      expect([...strip[0].querySelectorAll('.state-mark')].map((m) => m.className)).toEqual(
        cells.map((c) => `state-mark state-${c.state}`),
      );
      // The strip's own words, cell by cell.
      expect(strip[0].getAttribute('aria-label')).toBe(cells.map(cellNowWords).join('; '));
    }
  });

  it("explains the strip's marks by the works page's legend, on the work's tab", () => {
    const el = draw(12, answered([]));
    expect(el.querySelector('.legend')).toBeNull();
    act(() => button(el, 'Work').click());
    const legend = el.querySelector('.panel-part .legend');
    expect(legend?.getAttribute('aria-label')).toBe("What the marks of a cell's state mean");
    expect(text(el, '.legend li')).toEqual(['done', 'running', 'at rest', 'failed', 'runs again', 'not run yet', 'skipped']);
  });

  it('says on the work\'s line what the work waits for and who gives it, and a question a cell asked at the plan\'s end', () => {
    const el = draw(20, answered([]));
    const [, workLine] = lines(el);
    expect(workLine).toContain('Waiting for you until the owner confirmed the plan');
    expect(text(el, '.cw-choice-q')).toEqual(['May a page keep its stream open while hidden?']);
    expect(text(el, '.cw-choice-stops')).toEqual(['The work waits for this answer']);
  });

  it("says a message to the chosen cell's line, names the cell the engine says it reaches, and then the one it gave it to", async () => {
    const sent: Sent = [];
    // The engine gives the words to another cell than the one the page read it would: the page says the engine's.
    const el = draw(12, answered(sent, { reached: 'R1' }), undefined, true);
    // A button for each line that can ask the owner a question, and none for the review's line, which cannot.
    expect(text(el, '.cw-to .switch-choice')).toEqual(['D1', 'Implement']);
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['Implement']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Implement']);
    act(() => button(el, 'Implement').click());
    expect(text(el, '.write-readers')).toEqual(['Goes to Implement']);
    expect(button(el, 'Send').disabled).toBe(true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, '  Keep it to one table  ');
    expect(button(el, 'Send').disabled).toBe(false);
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'say', to: { kind: 'line', line: 'engineer' }, words: 'Keep it to one table' }]]);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
    expect(text(el, '.cw-done')).toEqual(['Said to Initial research']);
  });

  it('says words to the line where the work waits for an answer about that question, where the engine sends them', async () => {
    const sent: Sent = [];
    const i = itemOf(12);
    const waits = engineer({
      ...i,
      work: { ...i.work, now: { kind: 'waits', at: 'research', on: [{ kind: 'answer', question: 'one_table' }] } },
    } satisfies Item);
    const speaking: Speaking[] = [
      ...SPEAKING,
      { to: { kind: 'question', name: 'one_table' }, cell: 'research', refused: null },
    ];
    const el = draw(waits, answered(sent, { reached: 'R1' }), engineer(pageOf(i, speaking)));
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Why two tables?');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'say', to: { kind: 'question', name: 'one_table' }, words: 'Why two tables?' }]]);
    // To another line, they are said to that line, which the engine says no cell of can take them.
    act(() => button(el, 'D1').click());
    expect(text(el, '.write-readers')).toEqual([`No cell takes the words: ${SPEAKING[1].refused}`]);
  });

  it('shows a button for each line that can ask the owner a question and none for one that cannot, and says words to the line of the button chosen', async () => {
    const sent: Sent = [];
    const speaking: Speaking[] = [
      { to: { kind: 'line', line: 'engineer' }, cell: 'implement', refused: null },
      { to: { kind: 'line', line: 'deep_research' }, cell: 'deep_research', refused: null },
      { to: { kind: 'line', line: 'review' }, cell: 'review', refused: null },
    ];
    const i = itemOf(12);
    const el = draw(i, answered(sent, { reached: 'deep_research' }), pageOf(i, speaking));
    expect(text(el, '.cw-to .switch-choice')).toEqual(['Deep research', 'Implement']);
    act(() => button(el, 'Deep research').click());
    expect(text(el, '.write-readers')).toEqual(['Goes to Deep research']);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Look at the second table');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([
      [12, { kind: 'say', to: { kind: 'line', line: 'deep_research' }, words: 'Look at the second table' }],
    ]);
  });

  it('takes a question a cell asked by a call to the discussion, quoted, to the line of the cell that asked it', async () => {
    const sent: Sent = [];
    const speaking: Speaking[] = [{ to: { kind: 'line', line: 'engineer' }, cell: 'research', refused: null }];
    const el = draw(1, answered(sent, { reached: 'R1' }), engineer(pageOf(itemOf(1), speaking)), true);
    await act(async () => button(el, 'Discuss').click());
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['Initial research']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
    // The discussion's box, not the one the question is answered in.
    type(discussion(el).querySelector('textarea') as HTMLTextAreaElement, 'Kept a day: say why not.');
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([
      [
        1,
        {
          kind: 'say',
          to: { kind: 'line', line: 'engineer' },
          words: '> Should a stale run be abandoned at once or kept a day?\n\nKept a day: say why not.',
        },
      ],
    ]);
  });

  it('says why no cell can take a message, and sends it to no cell', async () => {
    const sent: Sent = [];
    const refused = 'no cell of the line engineer has run yet, and the work stands at none of them';
    const speaking: Speaking[] = [{ to: { kind: 'line', line: 'engineer' }, cell: null, refused }];
    const el = draw(12, answered(sent), engineer(pageOf(itemOf(12), speaking)), true);
    expect(text(el, '.write-readers')).toEqual([`No cell takes the words: ${refused}`]);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Keep it to one table');
    expect(button(el, 'Send').disabled).toBe(true);
    await act(async () => button(el, 'Send').click());
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

    draw(4, answered(sent));
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
      [4, { kind: 'resume' }],
      [4, { kind: 'cancel', why: 'Joined into #3' }],
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
    expect(text(el, '.pairs dd')).toEqual(['61', '$7.31', '1.1d ago', '0', 'engineer']);
  });

  it('waits for the work to be read before it offers the blocks to write in, and says so when it could not be', () => {
    const el = draw(12, answered([]), null);
    expect(el.querySelector('textarea')).toBeNull();
    expect(text(discussion(el), 'p')).toEqual(["Reading the work's flow"]);
    draw(12, answered([]), { failed: '/api/v1/works/12 answered 500' });
    expect(text(discussion(el), 'p')).toEqual(['The page could not read the work: /api/v1/works/12 answered 500']);
  });
});
