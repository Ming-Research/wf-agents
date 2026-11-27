// The system and the settings to look at their views by before the interface serves them, opened with `?fixtures`
// in the page's address (api.ts), and for the tests. The system is worked out from the works' fixtures, so that the two
// views agree: what each work spent, the one merged, the one cancelled, and a round failed of the work the engine
// paused. What was spent by cell, by model and by project is split from what the works spent in all; the projects are
// those the works' fixtures name, then made-up ones, as many as the address asks for, else three and the items of none.
// The settings are a file like the sample's, with a value the engine runs at another and a table left out, and four
// projects, each in a state the engine may hold its declaration in.

import { fixtureItems, fixtureProjects as worksProjects } from '../works/fixtures';
import type { Line, Spent } from '../works-api';
import type {
  AccountsPage,
  AccountUsage,
  Figure,
  Going,
  ProjectSettings,
  SettingsKey,
  SettingsPage,
  Occupancy,
  System,
  Week,
} from './api';

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

/// A part of what was spent, as a cell's, a model's or a project's share of it.
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

/// What is left of what was spent once a part of it is taken.
const less = (spent: Spent, taken: Spent): Spent => ({
  usdMicros: spent.usdMicros - taken.usdMicros,
  inputTokens: spent.inputTokens - taken.inputTokens,
  cacheWrittenTokens: spent.cacheWrittenTokens - taken.cacheWrittenTokens,
  unsplitTokens: spent.unsplitTokens - taken.unsplitTokens,
  cachedTokens: spent.cachedTokens - taken.cachedTokens,
  outputTokens: spent.outputTokens - taken.outputTokens,
  workingMs: spent.workingMs - taken.workingMs,
  rounds: spent.rounds - taken.rounds,
});

/// What was spent, split by these weights into parts that add up to it: each part is what the weights up to its end
/// take, less what those before it take.
function split(spent: Spent, weights: number[]): Spent[] {
  const total = weights.reduce((sum, w) => sum + w, 0);
  let before = 0;
  return weights.map((w) => {
    const from = part(spent, before / total);
    before += w;
    return less(part(spent, before / total), from);
  });
}

/// What was spent, by the projects the works' fixtures name, as many as `projects` asks for, the costliest first; where
/// no count is given, by as many as they name by default and, the least, the items of none.
function byProject(spent: Spent, projects: number | null): System['byProject'] {
  const named = worksProjects(projects ?? undefined).names;
  const all: (string | null)[] = projects === null ? [...named, null] : named;
  const parts = split(spent, all.map((project, n) => (project === null ? 0.05 : 1 / (n + 1))));
  return all
    .map((project, n) => ({ project, spent: parts[n] }))
    .sort((a, b) => b.spent.usdMicros - a.spent.usdMicros);
}

/// How the system went over these weeks, worked out from the works' fixtures, its spending split between as many
/// projects as `projects` asks for.
export function fixtureSystem(starts: number[], projects: number | null = null): System {
  const now = Date.now();
  const from = starts[0] ?? null;
  const items = fixtureItems().items;
  const spent = items.reduce((sum, i) => add(sum, i.work.spent), nothing());
  const merged = items.filter((i) => i.work.now.kind === 'merged');
  const cancelled = items.filter((i) => i.work.now.kind === 'cancelled');
  const halted = items.find((i) => i.work.now.kind === 'paused' && i.work.now.by === 'engine');
  const weeks = starts.length;
  const occupancy = (taking: number, running: number): Occupancy => ({
    takingAvg: taking,
    takingPeak: 3,
    runningAvg: running,
    runningPeak: 4,
    slots: 3,
    recorded: true,
  });
  const week = (n: number): Week => ({
    start: starts[n],
    spent: n === weeks - 1 ? spent : nothing(),
    merged: n === weeks - 1 ? merged.length : 0,
    cancelled: n === weeks - 1 ? cancelled.length : 0,
    added: n === weeks - 1 ? items.length : 0,
    sentBack: 0,
    failed: n === weeks - 1 && halted ? 1 : 0,
    waited: {
      totalMs: n === weeks - 1 ? [7_200_000, 1_800_000, 900_000, 600_000, 3_600_000] : [0, 0, 0, 0, 0],
      ciMs: n === weeks - 1 ? 1_500_000 : 0,
      occupancy: n === weeks - 1 ? occupancy(2400, 2900) : occupancy(0, 0),
    },
  });
  return {
    now,
    from,
    spent,
    weeks: Array.from({ length: weeks }, (_, n) => week(n)).reverse(),
    byProject: byProject(spent, projects),
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
    waited: {
      by: [
        { reason: 'owner', stops: 6, totalMs: 7_200_000, medianMs: 1_200_000, longestMs: 3_000_000 },
        { reason: 'items', stops: 2, totalMs: 1_800_000, medianMs: 900_000, longestMs: 1_200_000 },
        { reason: 'slot', stops: 9, totalMs: 900_000, medianMs: 60_000, longestMs: 300_000 },
        { reason: 'turn', stops: 1, totalMs: 600_000, medianMs: 600_000, longestMs: 600_000 },
        { reason: 'paused', stops: 3, totalMs: 3_600_000, medianMs: 1_200_000, longestMs: 2_400_000 },
      ],
      unseparated: { stops: 4, totalMs: 1_200_000, medianMs: 300_000, longestMs: 600_000 },
      ci: {
        stops: 5,
        totalMs: 1_500_000,
        medianMs: 240_000,
        longestMs: 540_000,
        jobs: 12,
        queuedMedianMs: 20_000,
        queuedLongestMs: 180_000,
      },
      occupancy: occupancy(2400, 2900),
      uncounted: 1,
    },
  };
}

/// What the calls of each agent's round under way in the works' fixtures have come to so far. The works' fixtures hold one
/// round under way, which shows each case of a figure in one row: the growth over the round past its line, the growth
/// for each call within its line, and the cost with no line to judge by yet; a round with no call told yet, as any other
/// under way would be, has no figure.
export function fixtureUnderWay(): Going[] {
  const rounds = fixtureItems().items.flatMap((i) =>
    i.work.now.kind === 'round' ? [{ work: i.work.number, run: i.work.now.run, round: i.work.now.round }] : [],
  );
  const line = (value: number, samples: number): Line => ({ value, samples, scope: 'cell' });
  const figure = (value: number, on: Line | null, past: boolean): Figure => ({ value, line: on, past });
  return rounds.map(
    (round, n): Going =>
      n === 0
        ? {
            ...round,
            context: 182_400,
            added: figure(96_000, line(60_000, 12), true),
            perCall: figure(3_000, line(5_000, 12), false),
            cost: figure(310_000, null, false),
          }
        : { ...round, context: null, added: null, perCall: null, cost: null },
  );
}

export function fixtureSettings(): SettingsPage {
  return {
    file: '/data/install.toml',
    base: 'fixture',
    unreadable: null,
    levels: ['1', '2', '3'],
    git: { committed: true },
    declarationFile: 'wf-agents.toml',
    projects: fixtureProjects(),
    tables: [
      { name: 'owner', leftOut: false, keys: [key('language', '"Chinese"', null, { kind: 'name' })] },
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
      {
        name: 'sessions',
        leftOut: true,
        keys: [{ key: 'compact_at', written: null, running: '500000', change: { kind: 'count' } }],
      },
    ],
  };
}

/// The projects: `wf-agents`, hosted on GitHub, whose declaration the engine read from the remote's main; `site`,
/// hosted in its own repository, whose declaration needs no variable and so writes `[environment]` with nothing in it;
/// `notes`, which the engine holds, its declaration not on its main; and `drafts`, added to the file after the engine
/// started, which the engine runs with none of and read no declaration of.
export function fixtureProjects(): ProjectSettings[] {
  const leftOut = (name: string, running: string) => ({ key: name, written: null, running, change: null });
  return [
    {
      name: 'wf-agents',
      tables: [
        {
          name: 'projects.wf-agents',
          leftOut: false,
          keys: [key('repo', '"/path/to/wf-agents"'), key('main', '"main"'), leftOut('allowed_config', '[]')],
        },
        {
          name: 'projects.wf-agents.github',
          leftOut: false,
          keys: [
            key('repository', '"owner/wf-agents"'),
            key('remote', '"upstream"'),
            key('runners', '["github", "14900k"]'),
          ],
        },
      ],
      declaration: {
        kind: 'read',
        at: 'refs/remotes/origin/main:wf-agents.toml',
        tables: [
          { name: '', leftOut: false, keys: [key('tree', '"design/"')] },
          { name: 'files.guide', leftOut: false, keys: [key('path', '"AGENTS.md"')] },
          { name: 'files.constitution', leftOut: false, keys: [key('path', '"design/constitution.md"')] },
          {
            name: 'project',
            leftOut: false,
            keys: [key('research', '"research/"'), key('test_names', '"a Rust test as nextest names it"')],
          },
          {
            name: 'tests',
            leftOut: false,
            keys: [key('paths', '[\n  ":(glob)crates/**/tests/**",\n  ":(glob)client/**/*.test.ts",\n]')],
          },
          {
            name: 'ci.fast',
            leftOut: false,
            keys: [key('workflow', '"fast.yml"'), key('runner', '"14900k"'), key('too_long', '"2h"')],
          },
          {
            name: 'ci.full',
            leftOut: false,
            keys: [key('workflows', '["ci.yml"]'), key('runner', '"14900k"'), key('too_long', '"2h"')],
          },
          {
            name: 'pull_request',
            leftOut: false,
            keys: [key('merge_method', '"squash"'), key('delete_branch', 'true')],
          },
          {
            name: 'protected',
            leftOut: false,
            keys: [key('paths', '["AGENTS.md", "design/constitution.md", "flow/", "wf-agents.toml"]')],
          },
          { name: 'heavy', leftOut: false, keys: [key('commands', '["cargo", "npm", "npx"]')] },
          { name: 'environment', leftOut: false, keys: [key('RUSTUP_AUTO_INSTALL', '"0"')] },
        ],
      },
    },
    {
      name: 'site',
      tables: [
        {
          name: 'projects.site',
          leftOut: false,
          keys: [key('repo', '"/path/to/site"'), key('main', '"trunk"'), leftOut('allowed_config', '[]')],
        },
      ],
      declaration: {
        kind: 'read',
        at: 'refs/heads/trunk:wf-agents.toml',
        tables: [
          { name: '', leftOut: false, keys: [key('tree', '"docs/design/"')] },
          { name: 'files.guide', leftOut: false, keys: [key('path', '"CONTRIBUTING.md"')] },
          { name: 'local', leftOut: false, keys: [key('check', '"./check.sh"')] },
          { name: 'protected', leftOut: false, keys: [key('paths', '["CONTRIBUTING.md", "wf-agents.toml"]')] },
          { name: 'heavy', leftOut: false, keys: [key('commands', '["make"]')] },
          { name: 'environment', leftOut: false, keys: [] },
        ],
      },
    },
    {
      name: 'notes',
      tables: [
        {
          name: 'projects.notes',
          leftOut: false,
          keys: [key('repo', '"/path/to/notes"'), key('main', '"main"'), leftOut('allowed_config', '[]')],
        },
      ],
      declaration: {
        kind: 'held',
        why:
          "refs/heads/main:wf-agents.toml, the project's declaration on main where works branch from, cannot be read: " +
          "fatal: path 'wf-agents.toml' does not exist in 'refs/heads/main'",
      },
    },
    {
      name: 'drafts',
      tables: [
        {
          name: 'projects.drafts',
          leftOut: false,
          keys: [key('repo', '"/path/to/drafts"', 'none'), key('main', '"main"', 'none')],
        },
      ],
      declaration: { kind: 'notRead' },
    },
  ];
}

function key(
  name: string,
  written: string,
  running: string | null = null,
  change: SettingsKey['change'] = null,
): SettingsKey {
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
