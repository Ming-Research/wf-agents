// The page at every width, as the UI spec lays it out: the bar never wider than the window, what of the engine it has
// no room for in its overflow, Start all or Pause all always in sight; and a split below the narrow breakpoint showing
// one side at a time, the whole width, the list until something is picked and what is picked under the way back to
// the list, the panel beside the one-to-one with an architect over the conversation, in its place, wherever the list,
// the conversation and the panel do not fit side by side.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import type { Conversation, Status } from './api';
import { shownConversation, sideOver, sideShows } from './pages/Messages';
import { ListSplit } from './parts';
import parts from './styles/parts.css?raw';
import screen from './styles/screen.css?raw';
import { t } from './text';
import { EngineBar, engineOverflow } from './TopBar';
import { widthOf } from './width';

vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useStatus: () => null,
  useResource: () => ({ data: undefined, error: null }),
}));

const status = (extra: Partial<Status> = {}): Status => ({
  paused: true,
  keep_warm: true,
  started: [30],
  releasing: [],
  waiting_for_slot: [],
  architects_started: [],
  running: null,
  stepping: null,
  last_step: null,
  slots: 2,
  slots_used: 1,
  flow_version: 2,
  config_commit: 'c',
  repos: {},
  started_at: 0,
  version: 'v',
  halt: null,
  ...extra,
});

const talk = (key: string): Conversation => ({
  key,
  kind: key.startsWith('work:') ? 'work' : 'architect',
  project: 'p',
  projects: [],
  work: null,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: null,
});

describe("the window's width", () => {
  it("is told apart at Primer's breakpoints: narrow below 768px, regular below 1280px, full from there on", () => {
    expect([375, 767, 768, 1012, 1279, 1280, 1400].map(widthOf)).toEqual([
      'narrow',
      'narrow',
      'regular',
      'regular',
      'regular',
      'full',
      'full',
    ]);
  });
});

describe("the bar's overflow of the engine", () => {
  // Its button, by Primer's mark of more, whose label Primer gives it as a tooltip.
  const MORE = 'octicon-kebab-horizontal';
  // Keep warm, by its part, since the button's label names it too.
  const WARM = 'class="keep-warm"';

  it('takes nothing at the full width, Keep warm and the slots below it, and the state in words too below the narrow breakpoint', () => {
    expect(engineOverflow('full')).toEqual([]);
    expect(engineOverflow('regular')).toEqual(['keep_warm', 'slots']);
    expect(engineOverflow('narrow')).toEqual(['words', 'keep_warm', 'slots']);
  });

  it('leaves Start all, or Pause all, and the dot of the state in the bar at every width', () => {
    for (const w of ['full', 'regular', 'narrow'] as const) {
      const paused = renderToStaticMarkup(<EngineBar status={status()} works={[]} width={w} />);
      expect(paused).toContain(t('start_all'));
      expect(paused).toContain('engine-dot-paused');
      expect(renderToStaticMarkup(<EngineBar status={status({ paused: false })} works={[]} width={w} />)).toContain(
        t('pause_all'),
      );
    }
  });

  it('holds Keep warm and the slots in the bar at the full width, with no overflow', () => {
    const bar = renderToStaticMarkup(<EngineBar status={status()} works={[]} width="full" />);
    expect(bar).toContain(WARM);
    expect(bar).toContain(t('slots', { used: 1, total: 2 }));
    expect(bar).toContain('engine-words');
    expect(bar).not.toContain(MORE);
  });

  it('puts Keep warm and the slots behind its button below the full width, the state in words staying', () => {
    const bar = renderToStaticMarkup(<EngineBar status={status()} works={[]} width="regular" />);
    expect(bar).toContain(MORE);
    expect(bar).not.toContain(WARM);
    expect(bar).not.toContain(t('slots', { used: 1, total: 2 }));
    expect(bar).toContain('engine-words');
  });

  it('puts the state in words behind it too below the narrow breakpoint, said on hovering the dot and to a reader', () => {
    const bar = renderToStaticMarkup(<EngineBar status={status()} works={[]} width="narrow" />);
    expect(bar).toContain(MORE);
    expect(bar).not.toContain('engine-words');
    expect(bar).not.toContain(WARM);
    const words = t('engine_paused_runs', { which: '#30' });
    expect(bar).toContain(`title="${words}`);
    expect(bar).toContain(`>${words}</span>`);
  });
});

describe('a split below the narrow breakpoint', () => {
  const split = (open: string | null) =>
    renderToStaticMarkup(
      <ListSplit label="Items" widthKey="k" back="/items" list={<p>the tree</p>} open={open !== null ? <p>{open}</p> : null} />,
    );

  it('shows the list alone while nothing is open, with no way back', () => {
    const html = split(null);
    expect(html).toMatch(/data-component="PageLayout.Content"[^>]*data-is-hidden="true"|data-is-hidden="true"[^>]*data-component="PageLayout.Content"/);
    expect(html).not.toContain('back-link');
  });

  it('shows what is open alone, the list hidden, under the way back to the list', () => {
    const html = split('item 2');
    const pane = html.slice(html.indexOf('PaneWrapper'), html.indexOf('>', html.indexOf('PaneWrapper')));
    expect(pane).toContain('data-is-hidden-narrow="true"');
    expect(pane).toContain('data-is-hidden-regular="false"');
    expect(html).toContain('back-link');
    expect(html).toContain('href="/items"');
    expect(html.indexOf('back-link')).toBeLessThan(html.indexOf('item 2'));
  });

  it('opens no conversation by itself: the list shows until the address names one', () => {
    const list = [talk('architect:p'), talk('work:3')];
    expect(shownConversation(list, null, true)).toBeNull();
    expect(shownConversation(list, 'work:3', true)?.key).toBe('work:3');
    expect(shownConversation(list, null, false)?.key).toBe('architect:p');
    expect(shownConversation(list, 'work:3', false)?.key).toBe('work:3');
  });

  it("puts the way back over the panel at a chart, which takes the chart's whole width, at Primer's narrow breakpoint", () => {
    const narrow = '@media (max-width: calc(48rem - 0.02px))';
    expect(screen.slice(screen.indexOf(narrow))).toMatch(/^@media \(max-width: calc\(48rem - 0\.02px\)\) \{\s*\.dock \{/);
    expect(parts).toMatch(/@media \(min-width: 48rem\) \{\s*\.back-link \{\s*display: none;/);
  });
});

describe('the panel beside the one-to-one with an architect', () => {
  it('is over the conversation, in its place, at every width but the full one', () => {
    expect(sideOver('narrow')).toBe(true);
    expect(sideOver('regular')).toBe(true);
    expect(sideOver('full')).toBe(false);
  });

  it('over the conversation, shows only while the address names one of its tabs, whatever the browser kept', () => {
    expect(sideShows(true, false, true)).toBe(false);
    expect(sideShows(true, true, false)).toBe(true);
    expect(sideShows(false, false, true)).toBe(true);
    expect(sideShows(false, false, false)).toBe(false);
  });
});
