import { describe, expect, it } from 'vitest';

import type { Anomaly, Merged, Status, Week, WorkSummary } from './api';
import { waitsWords } from './course';
import { anomaliesBy, byStanding, mergedAverages, spentIn, standingOf, systemPath } from './system';

const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
  id,
  item: id,
  title: 't',
  project: 'p',
  graph: 'engineer',
  flow_version: 2,
  moved_from: null,
  at: 'I1',
  at_title: 'Implement',
  stage: null,
  state: 'ready',
  paused: null,
  paused_by: null,
  ended: null,
  waits: null,
  branch: null,
  pr: null,
  started_at: null,
  last_at: null,
  rounds: 0,
  sent_back: 0,
  failed: 0,
  running: null,
  cost_usd: 0,
  open_asks: 0,
  open_design_cards: 0,
  ...extra,
});

const status = (extra: Partial<Status> = {}): Status => ({
  paused: false,
  keep_warm: true,
  started: [],
  releasing: [],
  architects_started: [],
  running: null,
  stepping: null,
  last_step: null,
  slots: 2,
  slots_used: 0,
  flow_version: 2,
  config_commit: 'c',
  repos: {},
  started_at: 0,
  version: 'v',
  halt: null,
  ...extra,
});

describe('the works by where they stand', () => {
  it('puts each work in one place, what needs the owner first', () => {
    const works = [
      work(1, { state: 'running' }),
      work(2),
      work(3, { state: 'asking', waits: { kind: 'answer', ask: 4 } }),
      // Paused by the owner and waiting for a ruling: the ruling is the owner's to give first.
      work(4, { state: 'paused', paused_by: 'owner', paused: 'later', waits: { kind: 'ruling', design_card: 2 } }),
      work(5, { state: 'asking', waits: { kind: 'architect', matter: 7 } }),
      work(6, { state: 'waiting', waits: { kind: 'items', items: [2] } }),
      work(7, { state: 'paused', paused_by: 'architect', paused: 'wait' }),
      work(8, { state: 'paused', paused_by: 'engine', paused: 'cell T2: red', waits: { kind: 'answer', ask: 9 } }),
      work(9, { state: 'done' }),
    ];
    const s = status({ running: 1 });
    expect(byStanding(works, s).map((g) => [g.standing, g.works.map((w) => w.id)])).toEqual([
      ['running', [1]],
      ['ready', [2]],
      ['you', [3, 4]],
      ['architect', [5]],
      ['items', [6]],
      ['paused', [7]],
      ['stopped', [8]],
    ]);
    // A work held only by every work's pause stands where it would without it; one the owner started and that waits for
    // a free slot is ready.
    expect(standingOf(work(2), status({ paused: true }))).toBe('ready');
    expect(standingOf(work(2, { paused_by: 'owner', paused: 'x' }), status({ releasing: [2] }))).toBe('ready');
    expect(standingOf(work(9, { state: 'ended' }), s)).toBe(null);
    expect(waitsWords({ kind: 'architect', matter: 7 })).toBe("waiting for the architect's reply to matter 7");
  });
});

describe('the System view', () => {
  it('asks for the weeks of its period in the owner’s time zone', () => {
    expect(systemPath('week', 480)).toBe('/api/v1/system?tz=480&weeks=1');
    expect(systemPath('4w', -300)).toBe('/api/v1/system?tz=-300&weeks=4');
    expect(systemPath('all', 0)).toBe('/api/v1/system?tz=0');
  });

  it('sums the weeks and the merges', () => {
    const week = (start: number, engineers_usd: number, architects_usd: number): Week => ({
      start,
      merged: 0,
      made: 0,
      cancelled: 0,
      open_prs: 0,
      engineers_usd,
      architects_usd,
    });
    expect(spentIn([week(0, 2, 1), week(1, 4, 0.5)])).toEqual({ engineers: 6, architects: 1.5, total: 7.5 });
    const merged = (started_at: number, merged_at: number, cost_usd: number): Merged => ({
      item: 1,
      title: 't',
      work: 1,
      started_at,
      merged_at,
      cost_usd,
    });
    expect(mergedAverages([])).toBe(null);
    expect(mergedAverages([merged(0, 100, 2), merged(50, 250, 4)])).toEqual({ took: 150, cost: 3 });
  });

  it('groups the anomalies by kind, in the order the view shows them', () => {
    const anomaly = (what: Anomaly['what']): Anomaly => ({
      work: 1,
      graph: 'engineer',
      item: 1,
      title: 't',
      project: 'p',
      round: 2,
      at: 0,
      what,
    });
    const groups = anomaliesBy([
      anomaly({ kind: 'main_red', how: 'broken_by_merge', checks: ['test'] }),
      anomaly({ kind: 'flow_anomaly', row: 'carry_on_unfit' }),
      anomaly({ kind: 'stopped', why: 'red' }),
    ]);
    expect(groups.map((g) => [g.kind, g.list.length])).toEqual([
      ['flow_anomaly', 1],
      ['counter', 0],
      ['stopped', 1],
      ['tool_failed', 0],
      ['main_red', 1],
    ]);
  });
});
