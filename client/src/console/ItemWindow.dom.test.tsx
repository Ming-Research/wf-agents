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
  page: { summary: i.work, title: i.title, flow: FLOW, cells: [], blocks: [], steps: [] } satisfies WorkPage,
});

/// What the panel sent, each answered at once.
type Sent = [number, Act][];
const answered = (sent: Sent) => (work: number, what: Act) => {
  sent.push([work, what]);
  return Promise.resolve(null);
};

/// Draws the panel of item `n` as the console does, in the same root as the panel drawn before it, if any.
function draw(n: number, send: (work: number, what: Act) => Promise<unknown>, page?: PageState) {
  const item = itemOf(n);
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
        page={page === undefined ? pageOf(item) : page}
        act={send}
        waitsFor={[11]}
        folded={false}
        onFold={() => undefined}
        onTitle={() => undefined}
      />,
    ),
  );
  return container;
}

const text = (el: HTMLElement, selector: string) =>
  [...el.querySelectorAll(selector)].map((e) => e.textContent?.replace(/\s+/g, ' ').trim() ?? '');

/// The words of each line, part by part.
const lines = (el: HTMLElement) =>
  [...el.querySelectorAll('.cw-line')].map((l) =>
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

/// The owner's answers to the confirms asked, one after another; happy-dom has no confirm of its own.
function confirms(...answers: boolean[]) {
  const ask = vi.fn(() => false);
  for (const a of answers) ask.mockReturnValueOnce(a);
  Object.defineProperty(window, 'confirm', { value: ask, configurable: true });
  return ask;
}

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
  it('heads with the number, the title and the flow, and says where the item stands and what its work does', () => {
    const el = draw(12, answered([]));
    expect(text(el, '.drawer-id')).toEqual(['12']);
    expect(text(el, '.drawer-title')).toEqual(['Show the cost of a work']);
    expect(text(el, '.drawer-belongs')).toEqual(['engineer']);
    const [itemLine, workLine] = lines(el);
    expect(itemLine).toBe('Open Added 1.1d ago Waits for 11 to merge');
    expect(workLine).toContain('Waiting for you at Review the plan W12');
    expect(el.querySelector('a.cw-mono')?.getAttribute('href')).toBe('#/works/12');
  });

  it('says a work the engine paused as halted, with the reason, and offers to let it on', () => {
    const el = draw(4, answered([]));
    const [, workLine] = lines(el);
    expect(workLine).toContain("Halted by the engine The agent's program exited with an error: the session could not be resumed.");
    expect(el.querySelectorAll('.cw-line .strip .state-failed')).toHaveLength(1);
    expect(button(el, 'Let it on')).toBeDefined();
  });

  it("shows the work's cells in the works page's strip, one mark per cell in the flow's order, each by its state", () => {
    for (const n of [12, 6, 4, 9, 2]) {
      const el = draw(n, answered([]));
      const cells = itemOf(n).work.cells;
      const strip = el.querySelectorAll('.cw-line .cw-progress .strip');
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

  it('says each thing the work waits for, and who gives it', () => {
    const el = draw(20, answered([]));
    expect(text(el, '.panel-part-label')).toEqual(['Waits at Implement 2']);
    expect(text(el, '.cw-wait')).toEqual([
      'You to answer: May a page keep its stream open while hidden?',
      'The owner confirmed the plan, which you make hold',
    ]);
  });

  it('writes a note into the block chosen, and names the cells that read it', async () => {
    const sent: Sent = [];
    const el = draw(12, answered(sent));
    const select = el.querySelector('select') as HTMLSelectElement;
    expect([...select.options].map((o) => o.value)).toEqual([
      'code_opinions',
      'plan_opinions',
      'plan_review_on_deep_research',
      'request',
      'test_opinions',
    ]);
    expect(text(el, '.cw-compose-foot .cw-muted')).toEqual(['Goes to Implement']);
    act(() => {
      select.value = 'request';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(text(el, '.cw-compose-foot .cw-muted')).toEqual(['Goes to Initial research']);
    expect(button(el, 'Send').disabled).toBe(true);
    type(el.querySelector('textarea') as HTMLTextAreaElement, '  Keep it to one table  ');
    expect(button(el, 'Send').disabled).toBe(false);
    await act(async () => button(el, 'Send').click());
    expect(sent).toEqual([[12, { kind: 'write', block: 'request', content: 'Keep it to one table', value: null }]]);
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
    expect(text(el, '.cw-done')).toEqual(['Written in request']);
  });

  it('keeps what is typed for a block of a work, so that it stands when the panel shows it again', () => {
    const el = draw(12, answered([]));
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'Half a thought');
    draw(2, answered([]));
    draw(12, answered([]));
    expect((el.querySelector('textarea') as HTMLTextAreaElement).value).toBe('Half a thought');
  });

  it('pauses a work that goes on, lets on one that is paused, and cancels one once the owner confirms', async () => {
    const sent: Sent = [];
    const el = draw(2, answered(sent));
    await act(async () => button(el, 'Pause').click());
    expect(sent).toEqual([[2, { kind: 'pause' }]]);

    draw(4, answered(sent));
    expect(button(el, 'Pause')).toBeUndefined();
    await act(async () => button(el, 'Let it on').click());
    const ask = confirms(false, true);
    await act(async () => button(el, 'Cancel').click());
    await act(async () => button(el, 'Cancel').click());
    expect(ask).toHaveBeenCalledTimes(2);
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
    expect(button(el, 'Pause').disabled).toBe(true);
    expect(button(el, 'Cancel').disabled).toBe(true);
    await act(async () => answers[0].yes());
    expect(button(el, 'Pause').disabled).toBe(false);
  });

  it('sends a note once: Send waits while it is under way', async () => {
    const { sent, answers, send } = held();
    const el = draw(12, send);
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
    expect(text(el, '.cw-refused')).toEqual(['The work has ended']);
    expect(button(el, 'Pause').disabled).toBe(false);
    // Another item: nothing refused there.
    draw(5, send);
    expect(text(el, '.cw-refused')).toEqual([]);
    // A note refused, then another item.
    draw(12, send);
    type(el.querySelector('textarea') as HTMLTextAreaElement, 'No');
    act(() => button(el, 'Send').click());
    await act(async () => answers[1].no('People do not write it'));
    expect(text(el, '.cw-refused')).toEqual(['People do not write it']);
    draw(20, send);
    expect(text(el, '.cw-refused')).toEqual([]);
  });

  it('offers nothing to do to a work that has ended', () => {
    const el = draw(18, answered([]));
    expect(el.querySelector('.cw-acts')).toBeNull();
    expect(lines(el)[1]).toContain('Merged');
  });

  it('says in its second tab what of the work the lines do not say', () => {
    const el = draw(2, answered([]));
    act(() => button(el, 'Work').click());
    expect(text(el, '.cw-facts dt')).toEqual(['Rounds', 'Cost', 'Started']);
    expect(text(el, '.cw-facts dd')).toEqual(['61', '$7.31', '1.3d ago']);
  });

  it('waits for the work to be read before it offers the blocks to write in, and says so when it could not be', () => {
    const el = draw(12, answered([]), null);
    expect(el.querySelector('select')).toBeNull();
    expect(text(el, '.cw-notes p')).toEqual(["Reading the work's flow"]);
    draw(12, answered([]), { failed: '/api/v1/works/12 answered 500' });
    expect(text(el, '.cw-notes p')).toEqual(['The page could not read the work: /api/v1/works/12 answered 500']);
  });
});
