// The list of conversations: wf-agents' own, then a group for each project, by name, headed by the buttons that show
// or hide its works in each state, the very part the items' tree shows or hides its items with, the done and the ended
// hidden at first and the conversation open kept whatever its state; then the groups of connected projects. What the
// buttons hide counts nowhere: not in the bar, nor on the row of the conversation open.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { Conversation, ItemState, WorkState, WorkSummary } from './api';
import { ITEM_MARK, ITEM_TONE, StateFilter, WORK_STATES, WORK_TONE, workStateMark } from './components';
import { rowsShown, totals } from './conversation';
import { counted, List, SHOWN } from './pages/Messages';
import { t, tn } from './text';

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: () => ({ data: undefined, error: null }),
}));

const talk = (key: string, extra: Partial<Conversation> = {}): Conversation => ({
  key,
  kind: key.startsWith('work:') ? 'work' : key.startsWith('architect:') ? 'architect' : key.startsWith('projects:') ? 'projects' : 'wf_agents',
  project: key.startsWith('architect:') ? key.slice('architect:'.length) : null,
  projects: [],
  work: null,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: 1,
  ...extra,
});

const workTalk = (id: number, project: string, over = false): Conversation =>
  talk(`work:${id}`, { project, work: id, item: id, title: `Work ${id}`, over });

/// A work as the works' list gives it, with only what the list of conversations reads of it.
const work = (id: number, state: WorkState): WorkSummary => ({ id, state, project: null }) as unknown as WorkSummary;

const list: Conversation[] = [
  talk('architect:q'),
  talk('architect:p'),
  talk('wf-agents'),
  workTalk(9, 'p'),
  workTalk(8, 'q', true),
  workTalk(7, 'p', true),
  workTalk(6, 'p'),
  workTalk(5, 'p', true),
  workTalk(4, 'q'),
  talk('projects:p|q', { projects: ['p', 'q'] }),
];

const works = [work(9, 'running'), work(8, 'done'), work(7, 'ended'), work(6, 'waiting'), work(5, 'done'), work(4, 'paused')];

/// The names of the rows and the headings of the list, in their order.
function read(html: string): string[] {
  return [...html.matchAll(/<h3[^>]*>([^<]+)<\/h3>|class="talks-title" title="([^"]+)"/g)].map((m) => m[1] ?? m[2]);
}

describe('the list of conversations', () => {
  it('shows each state of a work but done and ended at first', () => {
    expect(SHOWN).toEqual(['running', 'ready', 'asking', 'waiting', 'waiting_for_slot', 'paused', 'halted']);
  });

  it('groups by project, by name, each its architect then its works in the states shown; the open one stays', () => {
    const html = renderToStaticMarkup(<List list={list} open="work:5" works={works} />);
    expect(read(html)).toEqual([
      'wf-agents',
      'p',
      'Architect of p',
      '#9 Work 9',
      '#6 Work 6',
      '#5 Work 5',
      'q',
      'Architect of q',
      '#4 Work 4',
      'Connected projects',
      'Architects of p, q',
    ]);
    // The open one is marked as the list marks the one open, its state hidden.
    expect(html).toMatch(/aria-current="page"[^>]*href="\/messages\?c=work%3A5"|href="\/messages\?c=work%3A5"[^>]*aria-current="page"/);
  });

  it('heads each project with a button for each state of its works, with its count, pressed while shown', () => {
    const html = renderToStaticMarkup(<List list={list} open="architect:p" works={works} />);
    const buttons = [...html.matchAll(/<button[^>]*aria-pressed="(true|false)"[^>]*title="([^"]+)"/g)].map((m) => [m[2], m[1]]);
    expect(buttons).toEqual([
      ['1 work is running. Click to hide it.', 'true'],
      ['1 work is waiting. Click to hide it.', 'true'],
      ['1 work is done. Click to show it.', 'false'],
      ['1 work is ended. Click to show it.', 'false'],
      ['1 work is paused. Click to hide it.', 'true'],
      ['1 work is done. Click to show it.', 'false'],
    ]);
    expect(html).toContain('tree-state tree-tone-success');
    expect(read(html)).not.toContain('#5 Work 5');
  });

  it("draws each button as its state's mark and its count, with no words, the state and the count said as its name", () => {
    const filter = (shown: WorkState[]) =>
      renderToStaticMarkup(
        <StateFilter
          order={WORK_STATES}
          states={['running', 'asking', 'asking', 'done']}
          shown={shown}
          tone={WORK_TONE}
          mark={workStateMark}
          title={(s, n, on) => tn(on ? 'talks_hide' : 'talks_show', n, { state: t(`state_${s}`) })}
          onShown={() => undefined}
        />,
      );
    const html = filter(SHOWN);
    const buttons = [...html.matchAll(/<button([^>]*)>([\s\S]*?)<\/button>/g)];
    // Each button's text, its tags taken out, holds no words: its count alone, with the copy of it Primer's counter
    // keeps for a screen reader.
    const text = buttons.map((b) => b[2].replace(/<[^>]*>/g, ''));
    expect(text.map((x) => /[a-z]/i.test(x))).toEqual([false, false, false]);
    expect(text.map((x) => x.match(/\d+/)?.[0])).toEqual(['1', '2', '1']);
    expect(buttons.map((b) => /aria-label="([^"]+)"/.exec(b[1])?.[1])).toEqual([
      '1 work is running. Click to hide it.',
      '2 works are asking. Click to hide them.',
      '1 work is done. Click to show it.',
    ]);
    expect(buttons.map((b) => /title="([^"]+)"/.exec(b[1])?.[1])).toEqual(buttons.map((b) => /aria-label="([^"]+)"/.exec(b[1])?.[1]));
    // Running is the halo, the others their Octicons, as the rows' marks are.
    expect(buttons[0][2]).toContain('running-mark');
    expect(buttons[1][2]).toContain('octicon-question');
    expect(buttons[2][2]).toContain('octicon-check-circle');
  });

  it("draws the items' buttons with the items' marks", () => {
    const html = renderToStaticMarkup(
      <StateFilter
        order={['new', 'queued', 'working', 'done', 'stopped', 'cancelled'] as ItemState[]}
        states={['new', 'queued', 'queued', 'working', 'done', 'stopped', 'cancelled']}
        shown={['new', 'queued', 'working', 'stopped']}
        tone={ITEM_TONE}
        mark={(s) => ITEM_MARK[s]}
        title={(s, n, on) => tn(on ? 'items_hide' : 'items_show', n, { state: t(`item_${s}`) })}
        onShown={() => undefined}
      />,
    );
    const icons = [...html.matchAll(/class="octicon octicon-([\w-]+)"/g)].map((m) => m[1]);
    expect(icons).toEqual(['issue-opened', 'list-ordered', 'play', 'check-circle', 'stop', 'skip']);
    expect(html).toContain('title="2 items are queued. Click to hide them."');
    expect(html).toContain('title="1 item is cancelled. Click to show it."');
  });

  it('leaves out a work whose state is not read yet, but not the one open', () => {
    const stateOf = (c: Conversation) => (c.work === 9 ? 'running' : null);
    const p = list.filter((c) => c.project === 'p');
    expect(rowsShown(p, stateOf, SHOWN, null).map((c) => c.key)).toEqual(['architect:p', 'work:9']);
    expect(rowsShown(p, stateOf, SHOWN, 'work:6').map((c) => c.key)).toEqual(['architect:p', 'work:9', 'work:6']);
  });
});

describe('what the list hides counts nowhere', () => {
  const counts: Conversation[] = list.map((c) => ({ ...c, waiting: 1, unread: 2 }));

  it('sums in the bar only the conversations the list shows, whichever is open', () => {
    // Done 8 and 5 and ended 7 are hidden; the architects, wf-agents' own and the connected projects always count.
    expect(counted(counts, works, null, SHOWN).map((c) => c.key)).toEqual([
      'architect:q',
      'architect:p',
      'wf-agents',
      'work:9',
      'work:6',
      'work:4',
      'projects:p|q',
    ]);
    expect(totals(counted(counts, works, null, SHOWN))).toEqual({ waiting: 7, unread: 14 });
    // A state the owner shows counts again; a work whose state is not read yet counts once it is.
    expect(counted(counts, works, null, [...SHOWN, 'done']).map((c) => c.key)).toContain('work:5');
    expect(counted(counts, undefined, null, SHOWN).map((c) => c.key)).toEqual([
      'architect:q',
      'architect:p',
      'wf-agents',
      'projects:p|q',
    ]);
  });

  it('shows no counts on the row of the conversation open while its state is hidden', () => {
    const html = renderToStaticMarkup(<List list={counts} open="work:5" works={works} />);
    // The row of each conversation, from its title to the end of its item.
    const row = (title: string) => html.slice(html.indexOf(`title="${title}"`)).split('</li>')[0];
    expect(html).toContain('title="#5 Work 5"');
    expect(row('#5 Work 5')).not.toMatch(/waits for you|since you last read/);
    expect(row('#9 Work 9')).toContain('title="1 thing here waits for you"');
    expect(row('Architect of p')).toContain('title="2 new since you last read"');
  });
});
