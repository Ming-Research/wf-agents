// What the architect's page works out from what the interface reads, apart from drawing it: the wake it shows, how the
// architect stands, its queue in parts, the articles of the constitution in the order the owner reads them, and what an
// answer to an ask of the architect's sends.

import type { JsonValue, Matter, Wake, WorkSummary } from './api';
import { isObj, str, type Obj } from './labelled';

/// A wake goes on no more once it reached its end or was ended.
export const overWake = (w: WorkSummary): boolean => w.state === 'done' || w.state === 'ended';

/// The wake the page shows: the one the address names; else the one not over; else the latest; none before the first.
export function shownWake(wakes: Wake[], named: number | null): Wake | null {
  const asked = named !== null ? wakes.find((x) => x.work.id === named) : undefined;
  return asked ?? wakes.find((x) => !overWake(x.work)) ?? wakes[0] ?? null;
}

/// The architect's queue in its three parts: the matters due and waiting for it, in the order they came; those held
/// for the owner; and those not due yet.
export function queueParts(queue: Matter[]): { waiting: Matter[]; held: Matter[]; later: Matter[] } {
  return {
    waiting: queue.filter((m) => m.waits !== null),
    held: queue.filter((m) => m.waits === null && m.held !== null),
    later: queue.filter((m) => m.waits === null && m.held === null),
  };
}

export type ArchitectState =
  /// It never woke.
  | { kind: 'never'; waiting: number }
  /// Its last wake is over: since when, and the matter that falls due soonest.
  | { kind: 'asleep'; since: number | null; next: Matter | null; waiting: number }
  /// A wake goes on, handling a matter or none.
  | { kind: 'awake'; wake: WorkSummary; handling: JsonValue | null; waiting: number };

/// How the architect stands: never woken; asleep since its last wake ended, with the matter that falls due soonest; or
/// awake in a wake, with the matter it handles; and how many matters wait for it.
export function architectState(wakes: Wake[], queue: Matter[]): ArchitectState {
  const parts = queueParts(queue);
  const waiting = parts.waiting.length;
  const awake = wakes.find((w) => !overWake(w.work));
  if (awake !== undefined) return { kind: 'awake', wake: awake.work, handling: awake.handling, waiting };
  if (wakes.length === 0) return { kind: 'never', waiting };
  const next = parts.later.filter((m) => m.due !== null).sort((a, b) => (a.due ?? 0) - (b.due ?? 0))[0] ?? null;
  return { kind: 'asleep', since: wakes[0].work.last_at, next, waiting };
}

/// The choices an ask of the architect's offers, each value with what it means, as the engine read them from its cell.
export function choicesOf(body: JsonValue): Array<[string, string]> {
  const list = isObj(body) && Array.isArray(body.choices) ? body.choices : [];
  return list.flatMap((c) => (Array.isArray(c) && typeof c[0] === 'string' ? [[c[0], str(c[1])] as [string, string]] : []));
}

/// What an answer to a choice the architect put sends (the study of a new policy, the policy to approve): the value
/// chosen and the owner's words; none while it cannot be sent. Words are needed unless the choice is the first, the
/// one that agrees.
export function rulingAnswer(
  choices: Array<[string, string]>,
  choice: string | null,
  text: string,
): { choice: string; text: string } | null {
  if (choice === null || !choices.some(([v]) => v === choice)) return null;
  const words = text.trim();
  if (words === '' && choice !== choices[0][0]) return null;
  return { choice, text: words };
}

export type ChangesChoice = 'approve_all' | 'approve_some' | 'approve_none';

/// What an answer to the changes the architect collected sends: the choice, the changes approved by their numbers,
/// and why; none while it cannot be sent. Approving some needs some, not all, picked and why the rest are not;
/// approving none needs why.
export function changesAnswer(
  choice: ChangesChoice | null,
  picked: number[],
  all: number[],
  text: string,
): { choice: ChangesChoice; approved: number[]; text: string } | null {
  const why = text.trim();
  switch (choice) {
    case 'approve_all':
      return { choice, approved: [...all], text: why };
    case 'approve_none':
      return why !== '' ? { choice, approved: [], text: why } : null;
    case 'approve_some': {
      const approved = all.filter((n) => picked.includes(n));
      return approved.length > 0 && approved.length < all.length && why !== '' ? { choice, approved, text: why } : null;
    }
    default:
      return null;
  }
}

/// The articles of the constitution as the architect last checked them: those broken first, then those at risk, then
/// those kept, each in the order checked.
export function againstInOrder(rows: JsonValue): Obj[] {
  const rank = (s: string) => (s === 'broken' ? 0 : s === 'at_risk' ? 1 : 2);
  const list = Array.isArray(rows) ? rows.filter(isObj) : [];
  return list
    .map((r, i) => ({ r, i }))
    .sort((a, b) => rank(str(a.r.status)) - rank(str(b.r.status)) || a.i - b.i)
    .map((x) => x.r);
}

/// What one line of work became from one version of the state of things to the next: new, gone, changed or the same,
/// each of its fields with what it was and what it is.
export type LineChange = {
  line: string;
  kind: 'added' | 'removed' | 'changed' | 'same';
  fields: Array<{ field: string; before: string | null; after: string | null; changed: boolean }>;
};

/// The fields a line of work holds, in the order the page shows them.
export const LINE_FIELDS = ['line', 'serves', 'approach', 'where', 'stuck', 'recent'] as const;

/// What changed from one version of the state of things to the next, part by part: its lines of work, matched by
/// their name, in the new version's order with those gone after them; and what it says changed. With no version
/// before it, everything is new.
export function stateChanges(
  before: JsonValue | null,
  after: JsonValue,
): { lines: LineChange[]; changed: { before: string | null; after: string } } {
  const rows = (v: JsonValue | null): Obj[] => (isObj(v) && Array.isArray(v.lines) ? v.lines.filter(isObj) : []);
  const key = (o: Obj) => str(o.line).trim();
  const old = new Map(rows(before).map((o) => [key(o), o]));
  const now = rows(after);
  const seen = new Set(now.map(key));
  const fields = (o: Obj | undefined, n: Obj | undefined) =>
    LINE_FIELDS.map((f) => {
      const b = o !== undefined ? str(o[f]) : null;
      const a = n !== undefined ? str(n[f]) : null;
      return { field: f, before: b, after: a, changed: o !== undefined && n !== undefined && b !== a };
    });
  const lines: LineChange[] = now.map((n) => {
    const o = before === null ? undefined : old.get(key(n));
    const f = fields(o, n);
    const kind: LineChange['kind'] = o === undefined ? 'added' : f.some((x) => x.changed) ? 'changed' : 'same';
    return { line: key(n), kind, fields: f };
  });
  for (const [k, o] of old) {
    if (!seen.has(k)) lines.push({ line: k, kind: 'removed', fields: fields(o, undefined) });
  }
  return {
    lines,
    changed: { before: isObj(before) ? str(before.changed) : null, after: isObj(after) ? str(after.changed) : '' },
  };
}
