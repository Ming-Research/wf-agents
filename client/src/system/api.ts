// The system and the settings as the pages read them, as `crates/web/src/system.rs` and `crates/web/src/settings.rs`
// give them: the types here follow those, field for field. Every time is in milliseconds since the start of 1970.
//
// The routes:
// - `GET  /api/v1/system?weeks=`: how the system went over the weeks given, each by when it begins, the earliest first,
//   until now (`System`). The page cuts the weeks at Monday's midnight in the browser's time zone.
// - `GET  /api/v1/settings`: the installation's settings as their file writes them (`SettingsPage`).
// - `POST /api/v1/settings`: the owner's change of a few keys (`SettingsChange`), answered with what changed (`Saved`).
// - `GET  /api/v1/settings/accounts`: the accounts the agents' program may run under, as the engine holds them
//   (`AccountsPage`).
// - `POST /api/v1/settings/accounts`: the owner adds, takes or removes one (`AccountAsked`), answered with what came of
//   it (`AccountDone`). A token is sent once, as an account is added, and no answer holds one.
// - `POST /api/v1/settings/accounts/look`: the owner opens the accounts, and the engine looks under each whose usage it
//   last heard of long enough ago, answering at once with their names (`Looking`); the stream of changes says the
//   accounts changed as each look ends.
//
// The pages reach them through a context, which a test sets to its own source, and which the page's address with
// `?fixtures` sets to the fixtures (fixtures.ts).

import { createContext, useContext } from 'react';

import { read } from '../api';
import { usesFixtures } from '../mock-data';
import type { Against, Failure, Judged, Line, Refusal, Spent } from '../works-api';

export interface Week {
  /// When it begins, as the page gave it.
  start: number;
  /// What its rounds and its keep-warm pings took: `rounds` counts the rounds alone.
  spent: Spent;
  /// What the keep-warm pings that ended in it took, each counted as a round. Absent from an engine that keeps none.
  pinged?: Spent;
  merged: number;
  cancelled: number;
  added: number;
  sentBack: number;
  failed: number;
}

export interface CellSpent {
  flow: string;
  cell: string;
  spent: Spent;
}

/// What a round ran on: an agent's round's model, the engine's own act, which runs on none, or an agent's round kept
/// before the tries of a round were recorded, whose model is not known.
export type RanOn = { kind: 'model'; model: string } | { kind: 'act' } | { kind: 'notRecorded' };

export interface ModelSpent {
  on: RanOn;
  spent: Spent;
}

export interface WorkSpent {
  work: number;
  item: number;
  /// What its rounds and keep-warm pings that ended in the span took: `rounds` counts the rounds alone.
  spent: Spent;
}

export interface Merged {
  work: number;
  item: number;
  at: number;
  /// When its first round began.
  from: number | null;
  /// What all its rounds and keep-warm pings took: `rounds` counts the rounds alone.
  spent: Spent;
}

export interface Cancelled {
  work: number;
  item: number;
  at: number;
  why: string | null;
}

export interface Added {
  work: number;
  item: number;
  at: number;
}

export interface RoundAt {
  work: number;
  item: number;
  cell: string;
  run: number;
  round: number;
  began: number;
  spent: Spent | null;
}

export interface FailedRound {
  work: number;
  item: number;
  cell: string;
  run: number;
  round: number;
  ended: number;
  failure: Failure;
  why: string;
  spent: Spent;
}

/// How the system went over a span, the latest first in every list, the costliest first in every list by cost.
export interface System {
  now: number;
  /// When the span begins: its first week.
  from: number | null;
  /// What the span's rounds and keep-warm pings took: `rounds` counts the rounds alone.
  spent: Spent;
  /// What the keep-warm pings that ended in the span took, each counted as a round. Absent from an engine that keeps
  /// none.
  pinged?: Spent;
  weeks: Week[];
  byCell: CellSpent[];
  byModel: ModelSpent[];
  byWork: WorkSpent[];
  merged: Merged[];
  cancelled: Cancelled[];
  added: Added[];
  sentBack: RoundAt[];
  failed: FailedRound[];
  /// What ended in the span past the line of its history, the latest first.
  past: Past[];
  /// The rounds and the calls of tools under way now, the oldest first.
  underWay: UnderWay[];
  /// What the merged checks of each project say of how long its tests and jobs took. The page draws none of it: it is
  /// given in the data and by `wf-agents system`.
  tests?: TestTimes[];
}

/// What the merged checks of a project say of the time of its tests and jobs; each work named is the one whose merge
/// the thing was measured at.
export interface TestTimes {
  project: string | null;
  /// How many merged checks the project has.
  checks: number;
  /// The tests that took longer than their own history says in the latest merged check and in the one before.
  slowed: { work: number; unit: string; name: string; millis: number; median: number; line: number }[];
  /// The series of the CI's times that grew: `tests` is the sum of the tests' times, any other is a job's name.
  grown: { of: string; work: number; before: number; after: number; above: number; n: number; needed: number }[];
  /// The latest merges, the earliest first.
  merges: {
    work: number;
    jobs: { name: string; millis: number }[];
    testsMillis: number;
    added: number;
    addedMillis: number;
  }[];
}

/// What ended past the line of its history: a round whose cost or time went past its line, and a call of a tool whose
/// time or whose result's size did.
export type Past =
  | { kind: 'round'; work: number; item: number; cell: string; run: number; round: number; ended: number; judged: Judged }
  | {
      kind: 'call';
      work: number;
      item: number;
      cell: string;
      run: number;
      call: number;
      tool: string;
      done: number;
      time: Against | null;
      size: Against | null;
    };

/// What is under way, with how long it has taken so far against its line, where enough of its kind tell one, and an
/// agent's round with the other lines it is judged by as it runs.
export type UnderWay =
  | {
      kind: 'round';
      work: number;
      item: number;
      cell: string;
      run: number;
      round: number;
      began: number;
      time: Against | null;
      bounds: Bounds | null;
    }
  | {
      kind: 'call';
      work: number;
      item: number;
      cell: string;
      run: number;
      call: number;
      tool: string;
      made: number;
      time: Against | null;
    };

/// The lines an agent's round under way is judged by beside its time: what its context grows by over the round and
/// for each call after the first, and what it costs among the rounds on its model.
export interface Bounds {
  added: Line | null;
  rate: Line | null;
  cost: Line | null;
}

/// What kind of value the page may change a key to: a name on one line, a whole number, or one of the levels.
export type Change = { kind: 'name' } | { kind: 'count' } | { kind: 'level' };

export interface SettingsKey {
  /// The key within its table.
  key: string;
  /// Its value as the file writes it; none where the file leaves it out.
  written: string | null;
  /// The value the engine runs with, where it is not the one written; none where it is.
  running: string | null;
  change: Change | null;
}

export interface SettingsTable {
  /// Its name as its header writes it; empty for the keys before the first header.
  name: string;
  leftOut: boolean;
  keys: SettingsKey[];
}

export interface SettingsPage {
  file: string;
  /// The file's text as it was read, by a digest of it: a change is made against it.
  base: string;
  /// Why the file as it is now does not read as settings; none where it does.
  unreadable: string | null;
  tables: SettingsTable[];
  levels: string[];
  /// How the directory the file is in keeps it in git; none where it does not.
  git: { committed: boolean } | null;
}

export interface SettingsChange {
  base: string;
  set: { key: string; value: string | number }[];
}

export interface Saved {
  said: string;
  committed: boolean;
}

export interface Account {
  name: string;
  /// When it was added.
  added: number;
  /// Until when its token lasts.
  lasts: number;
  /// The usage limit the engine holds the account's rounds at, as its records say, where it stands: since when, and
  /// the reset it waits for, where one was said.
  limit: { since: number; resets: number | null } | null;
  /// What the agents' service last said of its usage.
  usage: AccountUsage;
}

/// What the agents' service last said of an account's usage, and when: each window it named, or, where it refused the
/// account's calls naming none, that the account is at its limit, with its reset where it said; whether a look under
/// it is under way; and why the latest look under it failed, where it did.
export interface AccountUsage {
  seen: number | null;
  windows: UsageWindow[];
  atLimit: { resets: number | null } | null;
  looking: boolean;
  failed: string | null;
}

/// A window of an account's usage, by the name the service gives it: how much of it is used, from 0 to 1, and when it
/// resets, where the service says; and whether the service lets the account's calls through in it without a warning.
export interface UsageWindow {
  name: string;
  utilization: number | null;
  resets: number | null;
  allowed: boolean;
}

export interface AccountsPage {
  /// The time now, by the engine's clock.
  now: number;
  /// The accounts added and not removed, in the order they were added.
  accounts: Account[];
  /// The account the agents run under; none where none is in use, and no agent's round runs.
  inUse: string | null;
  /// The command that makes a token.
  command: string;
  /// Why no account can be added now, as where what keeps the tokens is missing; none where one can.
  closed: string | null;
}

/// The accounts the engine looks under as the owner opens them.
export interface Looking {
  looking: string[];
}

/// What the owner does to an account.
export type AccountAsked =
  | { kind: 'add'; name: string; token: string }
  | { kind: 'take'; name: string }
  | { kind: 'remove'; name: string };

/// What came of it: of an account added, whether its token is at its usage limit and when that resets; of one removed,
/// why its token could not be let go of, where it could not.
export interface AccountDone {
  atLimit: boolean;
  resets: number | null;
  notLetGo: string | null;
}

async function send<T>(path: string, body: unknown): Promise<T> {
  const response = await fetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json' },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const refusal = (await response.json().catch(() => null)) as Refusal | null;
    throw new Error(refusal?.error ?? `${path} answered ${response.status}`);
  }
  return (await response.json()) as T;
}

/// What the system's and the settings' views read and send.
export interface SystemSource {
  /// How the system went over these weeks, each by when it begins, the earliest first.
  loadSystem(weeks: number[]): Promise<System>;
  loadSettings(): Promise<SettingsPage>;
  saveSettings(change: SettingsChange): Promise<Saved>;
  loadAccounts(): Promise<AccountsPage>;
  account(asked: AccountAsked): Promise<AccountDone>;
  /// The owner opens the accounts: the engine looks under those not heard of lately.
  look(): Promise<Looking>;
}

/// The interface.
export const INTERFACE: SystemSource = {
  loadSystem: (weeks) => read<System>(`/api/v1/system?weeks=${weeks.join(',')}`),
  loadSettings: () => read<SettingsPage>('/api/v1/settings'),
  saveSettings: (change) => send<Saved>('/api/v1/settings', change),
  loadAccounts: () => read<AccountsPage>('/api/v1/settings/accounts'),
  account: (asked) => send<AccountDone>('/api/v1/settings/accounts', asked),
  look: () => send<Looking>('/api/v1/settings/accounts/look', {}),
};

/// The fixtures, loaded only when asked for. What the owner changes is only logged.
export const FIXTURES: SystemSource = {
  loadSystem: async (weeks) => (await import('./fixtures')).fixtureSystem(weeks),
  loadSettings: async () => (await import('./fixtures')).fixtureSettings(),
  saveSettings: async (change) => {
    console.info('settings', change);
    return { said: change.set.map((s) => `${s.key} = ${JSON.stringify(s.value)}`).join('; '), committed: false };
  },
  loadAccounts: async () => (await import('./fixtures')).fixtureAccounts(),
  account: async (asked) => {
    // The token is logged nowhere.
    console.info('settings/accounts', asked.kind, asked.name);
    return { atLimit: false, resets: null, notLetGo: null };
  },
  look: async () => ({ looking: [] }),
};

export const SystemSourceContext = createContext<SystemSource>(
  typeof window === 'undefined' || !usesFixtures(window.location.search) ? INTERFACE : FIXTURES,
);

export function useSystemSource(): SystemSource {
  return useContext(SystemSourceContext);
}
