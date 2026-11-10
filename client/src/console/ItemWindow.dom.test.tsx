// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Panel } from '../parts';
import type { Act, Item, WorkPage } from '../works-api';
import { FLOW, fixtureItems } from './fixtures';
import { latest } from './items';
import { WindowBody, WindowLines, windowHead } from './ItemWindow';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
  vi.restoreAllMocks();
});

const ITEMS = fixtureItems();
const itemOf = (n: number) => ITEMS.items.find((i) => i.number === n) as Item;
const pageOf = (i: Item): WorkPage => ({
  summary: latest(i)!,
  title: i.title,
  flow: FLOW,
  cells: [],
  blocks: [],
  steps: [],
});

/// Draws the panel of item `n` as the console does, and gives its root and what it sent.
function draw(n: number, page: WorkPage | null = pageOf(itemOf(n))) {
  const item = itemOf(n);
  const sent: [number, Act][] = [];
  const send = (work: number, what: Act) => {
    sent.push([work, what]);
    return Promise.resolve(null);
  };
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <Panel
        label="item"
        {...windowHead(item, () => undefined)}
        lines={<WindowLines item={item} page={page} act={send} waitsFor={[11]} />}
        folded={false}
        onFold={() => undefined}
      >
        <WindowBody item={item} page={page} act={send} />
      </Panel>,
    ),
  );
  unmount = () => act(() => root.unmount());
  return { root: container, sent };
}

const text = (root: HTMLElement, selector: string) =>
  [...root.querySelectorAll(selector)].map((e) => e.textContent?.replace(/\s+/g, ' ').trim() ?? '');

/// The words of each line, part by part.
const lines = (root: HTMLElement) =>
  [...root.querySelectorAll('.cw-line')].map((l) =>
    [...l.children]
      .map((c) => c.textContent?.trim() ?? '')
      .filter((t) => t !== '')
      .join(' '),
  );

const button = (root: HTMLElement, words: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent === words) as HTMLButtonElement;

/// Types into a box as the browser would, so that React hears it.
function type(box: HTMLTextAreaElement, words: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set?.call(box, words);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

describe('the panel of the item selected', () => {
  it('heads with the number, the title and the flow, and says where the item stands and what its work does', () => {
    const { root } = draw(12);
    expect(text(root, '.drawer-id')).toEqual(['12']);
    expect(text(root, '.drawer-title')).toEqual(['Show the cost of a work']);
    expect(text(root, '.drawer-belongs')).toEqual(['engineer']);
    const [itemLine, workLine] = lines(root);
    expect(itemLine).toBe('Open Added 1.1d ago Waits for 11 to merge');
    expect(workLine).toContain('Waiting for you at Review the design W13');
    expect(root.querySelector('a.cw-mono')?.getAttribute('href')).toBe('#/works/13');
  });

  it('marks each cell of the flow: done, where the work waits, and the others', () => {
    const { root } = draw(12);
    const marks = [...root.querySelectorAll('.stage-cell')];
    expect(marks.map((m) => m.textContent).join('')).toBe('RDRITRCM');
    expect(marks.map((m) => m.className.replace('stage-cell stage-', ''))).toEqual([
      'done',
      'done',
      'wait',
      'ahead',
      'ahead',
      'ahead',
      'ahead',
      'ahead',
    ]);
    expect(marks[2].getAttribute('title')).toBe('Review the design: waits here');
  });

  it('says each thing the work waits for, and who gives it', () => {
    const { root } = draw(20);
    expect(text(root, '.panel-part-label')).toEqual(['Waits at Implement 2']);
    expect(text(root, '.cw-wait')).toEqual([
      'You to answer: May a page keep its stream open while hidden?',
      'The owner confirmed the plan, which you make hold',
    ]);
  });

  it('writes a note into the block chosen, and names the cells that read it', async () => {
    const { root, sent } = draw(12);
    const select = root.querySelector('select') as HTMLSelectElement;
    expect([...select.options].map((o) => o.value)).toEqual([
      'code_opinions',
      'design_opinions',
      'plan_opinions',
      'request',
      'test_opinions',
    ]);
    expect(text(root, '.cw-compose-foot .cw-muted')).toEqual(['Goes to Implement']);
    act(() => {
      select.value = 'request';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(text(root, '.cw-compose-foot .cw-muted')).toEqual(['Goes to Research the item']);
    expect(button(root, 'Send').disabled).toBe(true);
    type(root.querySelector('textarea') as HTMLTextAreaElement, '  Keep it to one table  ');
    expect(button(root, 'Send').disabled).toBe(false);
    await act(async () => button(root, 'Send').click());
    expect(sent).toEqual([[13, { kind: 'write', block: 'request', content: 'Keep it to one table', value: null }]]);
    expect((root.querySelector('textarea') as HTMLTextAreaElement).value).toBe('');
    expect(text(root, '.cw-done')).toEqual(['Written in request']);
  });

  it('keeps what is typed for a block of a work, so that it stands when the panel shows it again', () => {
    const first = draw(12);
    type(first.root.querySelector('textarea') as HTMLTextAreaElement, 'Half a thought');
    unmount?.();
    const again = draw(12);
    expect((again.root.querySelector('textarea') as HTMLTextAreaElement).value).toBe('Half a thought');
  });

  it('pauses a work that goes on, lets on one that is paused, and cancels one once the owner confirms', async () => {
    const running = draw(2);
    await act(async () => button(running.root, 'Pause').click());
    expect(running.sent).toEqual([[2, { kind: 'pause' }]]);
    unmount?.();

    const paused = draw(4);
    expect(button(paused.root, 'Pause')).toBeUndefined();
    await act(async () => button(paused.root, 'Let it on').click());
    // The owner says no to the first confirm, yes to the second; happy-dom has no confirm of its own.
    const ask = vi.fn(() => false).mockReturnValueOnce(false).mockReturnValueOnce(true);
    Object.defineProperty(window, 'confirm', { value: ask, configurable: true });
    await act(async () => button(paused.root, 'Cancel').click());
    await act(async () => button(paused.root, 'Cancel').click());
    expect(ask).toHaveBeenCalledTimes(2);
    expect(paused.sent).toEqual([
      [4, { kind: 'resume' }],
      [4, { kind: 'cancel' }],
    ]);
  });

  it('offers nothing to do to a work that has ended', () => {
    const { root } = draw(18);
    expect(root.querySelector('.cw-acts')).toBeNull();
    expect(lines(root)[1]).toContain('Merged');
  });

  it('says in its second tab what of the work the lines do not say', () => {
    const { root } = draw(2);
    act(() => button(root, 'Work').click());
    expect(text(root, '.cw-facts div')).toEqual(['Rounds61', 'Cost$7.31', 'Started1.3d ago']);
  });

  it('waits for the work to be read before it offers the blocks to write in', () => {
    const { root } = draw(12, null);
    expect(root.querySelector('select')).toBeNull();
    expect(text(root, '.cw-notes p')).toEqual(["Reading the work's flow"]);
  });
});
