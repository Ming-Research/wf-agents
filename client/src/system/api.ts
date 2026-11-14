// The system and the settings as the pages read them, as `crates/web/src/system.rs` and `crates/web/src/settings.rs`
// give them: the types here follow those, field for field. Every time is in milliseconds since the start of 1970.
//
// The routes:
// - `GET  /api/v1/system?weeks=`: how the system went over the weeks given, each by when it begins, the earliest first,
//   until now (`System`). The page cuts the weeks at Monday's midnight in the browser's time zone.
// - `GET  /api/v1/settings`: the installation's settings as their file writes them (`SettingsPage`).
// - `POST /api/v1/settings`: the owner's change of a few keys (`SettingsChange`), answered with what changed (`Saved`).
// - `GET  /api/v1/settings/accounts`: the accounts the agents' program may run under (`AccountsPage`).
// - `POST /api/v1/settings/accounts`: the owner's choice of another (`AccountChoice`), answered as a change.
//
// The pages reach them through a context, which a test sets to its own source, and which the page's address with
// `?fixtures` sets to the fixtures (fixtures.ts).

import { createContext, useContext } from 'react';

import { read } from '../api';
import { usesFixtures } from '../mock-data';
import type { Failure, Refusal, Spent } from '../works-api';

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
  path: string;
  signedInAs: string | null;
  /// Whether the file names it.
  named: boolean;
}

export interface AccountsPage {
  command: string;
  variable: string;
  signIn: string;
  /// The directory the accounts' directories are in; none where the file names no directory of the program's.
  dir: string | null;
  /// The name every account's directory begins with.
  stem: string;
  accounts: Account[];
  base: string;
}

export interface AccountChoice {
  base: string;
  name: string;
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
  chooseAccount(choice: AccountChoice): Promise<Saved>;
}

/// The interface.
export const INTERFACE: SystemSource = {
  loadSystem: (weeks) => read<System>(`/api/v1/system?weeks=${weeks.join(',')}`),
  loadSettings: () => read<SettingsPage>('/api/v1/settings'),
  saveSettings: (change) => send<Saved>('/api/v1/settings', change),
  loadAccounts: () => read<AccountsPage>('/api/v1/settings/accounts'),
  chooseAccount: (choice) => send<Saved>('/api/v1/settings/accounts', choice),
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
  chooseAccount: async (choice) => {
    console.info('settings/accounts', choice);
    return { said: `the agents run under ${choice.name}`, committed: false };
  },
};

export const SystemSourceContext = createContext<SystemSource>(
  typeof window === 'undefined' || !usesFixtures(window.location.search) ? INTERFACE : FIXTURES,
);

export function useSystemSource(): SystemSource {
  return useContext(SystemSourceContext);
}
