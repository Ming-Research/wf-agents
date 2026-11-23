// The system and the settings to look at their views by before the interface serves them, opened with `?fixtures`
// in the page's address (api.ts), and for the tests. The system is worked out from the works' fixtures, so that the two
// views agree: what each work spent, the one merged, the one cancelled, and a round failed of the work the engine
// paused. The settings are a file like the sample's, with a value the engine runs at another and a table left out.

import { fixtureItems } from '../works/fixtures';
import type { Spent } from '../works-api';
import type { AccountsPage, AccountUsage, SettingsPage, System, Week } from './api';

const nothing = (): Spent => ({
  usdMicros: 0,
  inputTokens: 0,
  cacheWrittenTokens: 0,
  unsplitTokens: 0,
  cachedTokens: 0,
  outputTokens: 0,
  workingMs: 0,
  rounds: 0,
});

const add = (a: Spent, b: Spent): Spent => ({
  usdMicros: a.usdMicros + b.usdMicros,
  inputTokens: a.inputTokens + b.inputTokens,
  cacheWrittenTokens: a.cacheWrittenTokens + b.cacheWrittenTokens,
  unsplitTokens: a.unsplitTokens + b.unsplitTokens,
  cachedTokens: a.cachedTokens + b.cachedTokens,
  outputTokens: a.outputTokens + b.outputTokens,
  workingMs: a.workingMs + b.workingMs,
  rounds: a.rounds + b.rounds,
});

/// A part of what was spent, as a cell's or a model's share of it.
const part = (spent: Spent, share: number): Spent => ({
  usdMicros: Math.round(spent.usdMicros * share),
  inputTokens: Math.round(spent.inputTokens * share),
  cacheWrittenTokens: Math.round(spent.cacheWrittenTokens * share),
  unsplitTokens: Math.round(spent.unsplitTokens * share),
  cachedTokens: Math.round(spent.cachedTokens * share),
  outputTokens: Math.round(spent.outputTokens * share),
  workingMs: Math.round(spent.workingMs * share),
  rounds: Math.round(spent.rounds * share),
});

export function fixtureSystem(starts: number[]): System {
  const now = Date.now();
  const from = starts[0] ?? null;
  const items = fixtureItems().items;
  const spent = items.reduce((sum, i) => add(sum, i.work.spent), nothing());
  const merged = items.filter((i) => i.work.now.kind === 'merged');
  const cancelled = items.filter((i) => i.work.now.kind === 'cancelled');
  const halted = items.find((i) => i.work.now.kind === 'paused' && i.work.now.by === 'engine');
  const weeks = starts.length;
  const week = (n: number): Week => ({
    start: starts[n],
    spent: n === weeks - 1 ? spent : nothing(),
    merged: n === weeks - 1 ? merged.length : 0,
    cancelled: n === weeks - 1 ? cancelled.length : 0,
    added: n === weeks - 1 ? items.length : 0,
    sentBack: 0,
    failed: n === weeks - 1 && halted ? 1 : 0,
  });
  return {
    now,
    from,
    spent,
    weeks: Array.from({ length: weeks }, (_, n) => week(n)).reverse(),
    byCell: [
      { flow: 'engineer', cell: 'implement', spent: part(spent, 0.5) },
      { flow: 'engineer', cell: 'review', spent: part(spent, 0.3) },
      { flow: 'engineer', cell: 'plan', spent: part(spent, 0.2) },
    ],
    byModel: [
      { on: { kind: 'model', model: 'claude-sonnet-5-5' }, spent: part(spent, 0.8) },
      { on: { kind: 'model', model: 'claude-opus-5-5' }, spent: part(spent, 0.2) },
      { on: { kind: 'act' }, spent: part(spent, 0) },
    ],
    byWork: items
      .filter((i) => i.work.spent.rounds > 0)
      .map((i) => ({ work: i.work.number, item: i.number, spent: i.work.spent }))
      .sort((a, b) => b.spent.usdMicros - a.spent.usdMicros),
    merged: merged.map((i) => ({
      work: i.work.number,
      item: i.number,
      at: i.work.last ?? now,
      from: i.work.from ?? null,
      spent: i.work.spent,
    })),
    cancelled: cancelled.map((i) => ({
      work: i.work.number,
      item: i.number,
      at: i.work.last ?? now,
      why: 'The chart is drawn another way.',
    })),
    added: items.map((i) => ({ work: i.work.number, item: i.number, at: i.added })).sort((a, b) => b.at - a.at),
    sentBack: [],
    failed: halted
      ? [
          {
            work: halted.work.number,
            item: halted.number,
            cell: 'implement',
            run: 4,
            round: 2,
            ended: halted.work.last ?? now,
            failure: 'other',
            why: "The agent's program could not start.",
            spent: nothing(),
          },
        ]
      : [],
    past: [],
    underWay: [],
  };
}

export function fixtureSettings(): SettingsPage {
  return {
    file: '/data/install.toml',
    base: 'fixture',
    unreadable: null,
    levels: ['1', '2', '3'],
    git: { committed: true },
    tables: [
      { name: 'owner', leftOut: false, keys: [key('language', '"Chinese"', null, { kind: 'name' })] },
      {
        name: 'project',
        leftOut: false,
        keys: [key('repo', '"/path/to/the/repository"'), key('main', '"main"'), key('check', '"./test.sh"')],
      },
      {
        name: 'agents',
        leftOut: false,
        keys: [
          key('command', '"claude"'),
          key('slots', '3', '2', { kind: 'count' }),
          { key: 'highest_level', written: null, running: null, change: { kind: 'level' } },
          key('env', '["HTTPS_PROXY"]'),
          key('work_budget_usd', '5.0'),
        ],
      },
      { name: 'agents.levels.1', leftOut: false, keys: levelKeys('Easy work', 'claude-haiku-5-5') },
      { name: 'agents.levels.2', leftOut: false, keys: levelKeys('Work of ordinary difficulty', 'claude-sonnet-5-5') },
      { name: 'agents.levels.3', leftOut: false, keys: levelKeys('Hard work', 'claude-opus-5-5') },
      {
        name: 'items',
        leftOut: false,
        keys: [key('default_level', '"2"'), key('highest_level', '"3"', null, { kind: 'level' }), key('title_max', '20')],
      },
      { name: 'github', leftOut: false, keys: [key('repository', '"owner/name"'), key('token_env', '"WF_GITHUB_TOKEN"')] },
      {
        name: 'sessions',
        leftOut: true,
        keys: [{ key: 'compact_at', written: null, running: '500000', change: { kind: 'count' } }],
      },
    ],
  };
}

function key(name: string, written: string, running: string | null = null, change: SettingsPage['tables'][number]['keys'][number]['change'] = null) {
  return { key: name, written, running, change };
}

function levelKeys(desc: string, model: string) {
  return [key('desc', JSON.stringify(desc)), key('model', JSON.stringify(model), null, { kind: 'name' })];
}

/// The accounts: `work`, in use, whose week and five hours the service named three minutes ago; `home`, which the
/// engine holds at its weekly limit; and `old`, whose token ends within a month and of whose usage nothing is known.
export function fixtureAccounts(): AccountsPage {
  const now = Date.UTC(2026, 10, 19, 12);
  const day = 24 * 60 * 60 * 1000;
  const hour = 60 * 60 * 1000;
  const unknown: AccountUsage = { seen: null, windows: [], atLimit: null, looking: false, failed: null };
  const account = (
    name: string,
    addedDaysAgo: number,
    usage: AccountUsage = unknown,
    limit: AccountsPage['accounts'][number]['limit'] = null,
  ) => ({
    name,
    added: now - addedDaysAgo * day,
    lasts: now - addedDaysAgo * day + 365 * day,
    limit,
    usage,
  });
  const work: AccountUsage = {
    seen: now - 3 * 60 * 1000,
    windows: [
      { name: 'five_hour', utilization: 0.2, resets: now + 2 * hour, allowed: true },
      { name: 'seven_day', utilization: 0.67, resets: now + 4 * day, allowed: true },
    ],
    atLimit: null,
    looking: false,
    failed: null,
  };
  const home: AccountUsage = {
    seen: now - 2 * hour,
    windows: [{ name: 'seven_day', utilization: 1, resets: now + 3 * hour, allowed: false }],
    atLimit: null,
    looking: false,
    failed: null,
  };
  return {
    now,
    accounts: [
      account('work', 40, work),
      account('home', 20, home, { since: now - 2 * hour, resets: now + 3 * hour }),
      account('old', 350),
    ],
    inUse: 'work',
    movingOn: false,
    moves: [],
    command: 'claude setup-token',
    closed: null,
  };
}
