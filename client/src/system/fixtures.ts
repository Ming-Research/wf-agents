// The system and the settings to look at their views by before the interface serves them, opened with `?fixtures`
// in the page's address (api.ts), and for the tests. The system is worked out from the works' fixtures, so that the two
// views agree: what each work spent, the one merged, the one cancelled, and a round failed of the work the engine
// paused. The settings are a file like the sample's, with a value the engine runs at another and a table left out.

import { fixtureItems } from '../works/fixtures';
import type { Spent } from '../works-api';
import type { AccountsPage, SettingsPage, System, Week } from './api';

const nothing = (): Spent => ({
  usdMicros: 0,
  inputTokens: 0,
  cachedTokens: 0,
  outputTokens: 0,
  workingMs: 0,
  rounds: 0,
});

const add = (a: Spent, b: Spent): Spent => ({
  usdMicros: a.usdMicros + b.usdMicros,
  inputTokens: a.inputTokens + b.inputTokens,
  cachedTokens: a.cachedTokens + b.cachedTokens,
  outputTokens: a.outputTokens + b.outputTokens,
  workingMs: a.workingMs + b.workingMs,
  rounds: a.rounds + b.rounds,
});

/// A part of what was spent, as a cell's or a model's share of it.
const part = (spent: Spent, share: number): Spent => ({
  usdMicros: Math.round(spent.usdMicros * share),
  inputTokens: Math.round(spent.inputTokens * share),
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
      { model: 'claude-sonnet-5-5', spent: part(spent, 0.8) },
      { model: 'claude-opus-5-5', spent: part(spent, 0.2) },
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
      { name: 'owner', leftOut: false, keys: [key('language', '"Chinese"', null, { kind: 'name', most: 40 })] },
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
          key('config_dir', '"/data/agents/claude"'),
          key('slots', '3', '2', { kind: 'count', least: 1, most: 32 }),
          key('args', '["--permission-mode", "acceptEdits"]'),
          key('work_budget_usd', '5.0'),
        ],
      },
      { name: 'agents.levels.1', leftOut: false, keys: levelKeys('Easy work', 'claude-haiku-5-5') },
      { name: 'agents.levels.2', leftOut: false, keys: levelKeys('Work of ordinary difficulty', 'claude-sonnet-5-5') },
      { name: 'agents.levels.3', leftOut: false, keys: levelKeys('Hard work', 'claude-opus-5-5') },
      {
        name: 'items',
        leftOut: false,
        keys: [key('default_level', '"2"'), key('highest_level', '"3"', null, { kind: 'level' })],
      },
      { name: 'github', leftOut: false, keys: [key('repository', '"owner/name"'), key('token_env', '"WF_GITHUB_TOKEN"')] },
      {
        name: 'sessions',
        leftOut: true,
        keys: [{ key: 'context_limit', written: null, running: '500000', change: { kind: 'count', least: 50000, most: 1000000 } }],
      },
    ],
  };
}

function key(name: string, written: string, running: string | null = null, change: SettingsPage['tables'][number]['keys'][number]['change'] = null) {
  return { key: name, written, running, change };
}

function levelKeys(desc: string, model: string) {
  return [key('desc', JSON.stringify(desc)), key('model', JSON.stringify(model), null, { kind: 'name', most: 100 })];
}

export function fixtureAccounts(): AccountsPage {
  return {
    command: 'claude',
    variable: 'CLAUDE_CONFIG_DIR',
    signIn: '/login',
    dir: '/data/agents',
    stem: 'claude',
    base: 'fixture',
    accounts: [
      { name: 'claude', path: '/data/agents/claude', signedInAs: 'owner@example.com', named: true },
      { name: 'claude-second', path: '/data/agents/claude-second', signedInAs: 'second@example.com', named: false },
      { name: 'claude-third', path: '/data/agents/claude-third', signedInAs: null, named: false },
    ],
  };
}
