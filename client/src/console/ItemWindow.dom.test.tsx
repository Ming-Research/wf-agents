// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { type Root, createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { cellNowWords } from '../works/marks';
import type { Act, Item, WorkPage } from '../works-api';
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
const itemOf = (n: number) => ITEMS.items.find((i) => i.number === n) as Item;
const pageOf = (i: Item): PageState => ({
  page: {
    summary: i.work,
    title: i.title,
    level: i.level,
    project: i.project,
    history: i.history,
    flow: FLOW,
    cells: [],
    blocks: [],
    steps: [],
  } satisfies WorkPage,
});

/// The same, with the engineer's flow's own cell ids (R1, D1, I1) in place of the made-up ones, which the discussion
/// writes to by those ids.
const ENGINEER: Record<string, string> = { research: 'R1', deep_research: 'D1', implement: 'I1' };
const engineer = <T,>(x: T): T =>
  JSON.parse(JSON.stringify(x).replace(/"(research|deep_research|implement)"/g, (_, id: string) => `"${ENGINEER[id]}"`)) as T;

/// What the panel sent, each answered at once.
type Sent = [number, Act][];
const answered = (sent: Sent) => (work: number, what: Act) => {
  sent.push([work, what]);
  return Promise.resolve(null);
};

/// Draws the panel of item `n` as the console does, in the same root as the panel drawn before it, if any; `asEngineer`
/// draws it with the engineer's flow's cell ids.
function draw(n: number, send: (work: number, what: Act) => Promise<unknown>, page?: PageState, asEngineer = false) {
  const item = asEngineer ? engineer(itemOf(n)) : itemOf(n);
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
        page={page === undefined ? (asEngineer ? engineer(pageOf(itemOf(n))) : pageOf(item)) : page}
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

  it('writes a message into the block that is the owner\'s words to the cell chosen, and names the cells that read it', async () => {
    const sent: Sent = [];
    const el = draw(12, answered(sent), undefined, true);
    expect(text(el, '.cw-to .switch-choice')).toEqual(['Research', 'Deep research', 'Implementation']);
    // Before the work has come to the implementation, a message goes to the research.
    expect(text(el, '.cw-to [aria-checked="true"]')).toEqual(['Research']);
    expect(text(el, '.write-readers')).toEqual(['Goes to Initial research']);
    act(() => button(el, 'Implementation').click());
    expect(text(el, '.write-readers')).toEqual(['Goes to Implement']);
    expect(button(el, 'Send').disabled).toBe(true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, '  Keep it to one table  ');
    expect(button(el, 'Send').disabled).toBe(false);
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'write', block: 'code_opinions', content: 'Keep it to one table', value: null }]]);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
    expect(text(el, '.cw-done')).toEqual(['Written in code_opinions']);
  });

  it('keeps what is typed for a block of a work, so that it stands when the panel shows it again', () => {
    const el = draw(12, answered([]), undefined, true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Half a thought');
    draw(2, answered([]), undefined, true);
    draw(12, answered([]), undefined, true);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('Half a thought');
  });

  it('pauses a work that goes on, starts one that is paused, and cancels one once the owner confirms', async () => {
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
    await act(async () => button(el, 'Cancel the work').click());
    expect(sent.slice(1)).toEqual([
      [4, { kind: 'resume' }],
      [4, { kind: 'cancel' }],
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
