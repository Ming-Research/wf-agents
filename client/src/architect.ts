// What an architect's page works out from what the interface reads, apart from drawing it: the wake it shows, how the
// architect stands, its queue in parts, why the owner's words still wait for it, the observations the owner looks for,
// the articles of the constitution in the order the owner reads them, and what an answer to an ask of the architect's
// sends.

import type { ConversationEntry, JsonValue, Matter, Observation, OwnerAsk, Status, Wake, WorkSummary } from './api';
import { isObj, str, type Obj } from './labelled';

/// A wake goes on no more once it reached its end or was ended.
export const overWake = (w: WorkSummary): boolean => w.state === 'done' || w.state === 'ended';

/// The wake the page shows: the one the address names; else the one not over; else the latest; none before the first.
export function shownWake(wakes: Wake[], named: number | null): Wake | null {
  const asked = named !== null ? wakes.find((x) => x.work.id === named) : undefined;
  return asked ?? wakes.find((x) => !overWake(x.work)) ?? wakes[0] ?? null;
}

/// An architect's queue in its two parts: the matters in its queue, in the order they came; and those an ask of the
/// owner holds until answered.
export function queueParts(queue: Matter[]): { waiting: Matter[]; held: Matter[] } {
  const held = (m: Matter) => m.asked !== null && !m.answered;
  return { waiting: queue.filter((m) => !held(m)), held: queue.filter(held) };
}

export type ArchitectState =
  /// It never woke.
  | { kind: 'never'; waiting: number }
  /// Its last wake is over, since when.
  | { kind: 'asleep'; since: number | null; waiting: number }
  /// A wake goes on, handling a matter or none.
  | { kind: 'awake'; wake: WorkSummary; handling: Matter | null; waiting: number };

/// How an architect stands: never woken; asleep since its last wake ended; or awake in a wake, with the matter it
/// handles; and how many matters are in its queue.
export function architectState(wakes: Wake[], queue: Matter[]): ArchitectState {
  const waiting = queueParts(queue).waiting.length;
  const awake = wakes.find((w) => !overWake(w.work));
  if (awake !== undefined) {
    const handling = awake.handling !== null ? (queue.find((m) => m.id === awake.handling) ?? null) : null;
    return { kind: 'awake', wake: awake.work, handling, waiting };
  }
  if (wakes.length === 0) return { kind: 'never', waiting };
  return { kind: 'asleep', since: wakes[0].work.last_at, waiting };
}

/// Why the owner's words still wait for the architect, while some do: its wake is paused; it is held back while every
/// work is paused and the owner has not started it; it is asleep, and wakes for them by itself; or it is awake and
/// takes them up in its turn. None while no words wait.
export function wordsWait(
  talk: ConversationEntry[],
  state: ArchitectState,
  status: Status | null,
  project: string,
): 'paused' | 'held' | 'asleep' | 'awake' | null {
  if (!talk.some((x) => x.pending && x.sender === 'owner')) return null;
  if (state.kind === 'awake' && state.wake.paused !== null) return 'paused';
  if (status !== null && status.paused && !status.architects_started.includes(project)) return 'held';
  return state.kind === 'awake' ? 'awake' : 'asleep';
}

/// The tab of the architect's panel an ask of the owner is in, by the stage that put it: the policy's for the study of a
/// new policy and its approval, which the cells that set a policy put; the asks' for every other.
export function askTab(a: OwnerAsk): 'policy' | 'asks' {
  const kind = isObj(a.body) ? str(a.body.kind) : '';
  return kind === 'study' || kind === 'policy_approval' ? 'policy' : 'asks';
}

/// The observations the owner looks for: with the tag chosen, if one is, and the words searched for, if any, in what
/// it is, where it came from or what it refers to; those closed only when asked for. The order stays as read: the
/// open ones first, each newest first.
export function observationsShown(all: Observation[], tag: string, search: string, closed: boolean): Observation[] {
  const words = search.trim().toLowerCase();
  return all.filter(
    (o) =>
      (closed || o.closed === null) &&
      (tag === '' || o.tags.includes(tag)) &&
      (words === '' || [o.what, o.source, ...o.refs].some((x) => x.toLowerCase().includes(words))),
  );
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
export const LINE_FIELDS = ['line', 'serves', 'approach', 'where', 'stuck', 'recent', 'grounds'] as const;

/// How alike two lines of work are by what they say, from 0 to 1: the share of the words they have in common, all
/// fields together, of the words either has. A word is one of three letters or more in a script that spaces its words,
/// and each two characters in a row of Chinese, which does not; so a line written again in other words still comes out
/// close in either.
export function lineLikeness(a: Obj, b: Obj): number {
  const words = (o: Obj) => {
    const text = LINE_FIELDS.map((f) => str(o[f]))
      .join(' ')
      .toLowerCase();
    const out = new Set<string>();
    for (const w of text.match(/[\p{L}\p{N}]+/gu) ?? []) {
      const chars = [...w];
      if (/\p{Script=Han}/u.test(w)) {
        for (let i = 0; i + 1 < chars.length; i++) out.add(chars[i] + chars[i + 1]);
        if (chars.length === 1) out.add(w);
      } else if (chars.length >= 3) {
        out.add(w);
      }
    }
    return out;
  };
  const x = words(a);
  const y = words(b);
  if (x.size === 0 || y.size === 0) return 0;
  let both = 0;
  for (const w of x) if (y.has(w)) both += 1;
  return both / (x.size + y.size - both);
}

/// How alike two lines must be, by [`lineLikeness`], for a line of a new version that has no line of its name in the
/// version before to be taken for one of that version's written again; the likest pairs are taken first. Lines of one
/// project written apart share a few words, a line written again in other words shares more.
export const SAME_LINE = 0.15;

/// What changed from one version of the understanding to the next, part by part: the lines of work of its state of
/// things, each matched to the line of the version before of its name, or else to the one most like it by what it says,
/// so that a line written again in other words reads as changed and not as gone and new; in the new version's order
/// with those gone after them; and what it says changed. With no version before it, everything is new.
export function stateChanges(
  before: JsonValue | null,
  after: JsonValue,
): { lines: LineChange[]; changed: { before: string | null; after: string } } {
  const rows = (v: JsonValue | null): Obj[] => (isObj(v) && Array.isArray(v.state) ? v.state.filter(isObj) : []);
  const key = (o: Obj) => str(o.line).trim();
  const olds = rows(before);
  const now = rows(after);
  // Which line of the version before each new line is, by its index: the same name first, then the likest by content.
  const of = new Map<number, number>();
  const taken = new Set<number>();
  now.forEach((n, i) => {
    const j = olds.findIndex((o, k) => !taken.has(k) && key(o) === key(n));
    if (j >= 0) {
      of.set(i, j);
      taken.add(j);
    }
  });
  const alike: Array<{ i: number; j: number; s: number }> = [];
  now.forEach((n, i) => {
    if (of.has(i)) return;
    olds.forEach((o, j) => {
      if (taken.has(j)) return;
      const s = lineLikeness(o, n);
      if (s >= SAME_LINE) alike.push({ i, j, s });
    });
  });
  alike.sort((a, b) => b.s - a.s);
  for (const x of alike) {
    if (of.has(x.i) || taken.has(x.j)) continue;
    of.set(x.i, x.j);
    taken.add(x.j);
  }
  const fields = (o: Obj | undefined, n: Obj | undefined) =>
    LINE_FIELDS.map((f) => {
      const b = o !== undefined ? str(o[f]) : null;
      const a = n !== undefined ? str(n[f]) : null;
      return { field: f, before: b, after: a, changed: o !== undefined && n !== undefined && b !== a };
    });
  const lines: LineChange[] = now.map((n, i) => {
    const j = of.get(i);
    const o = j !== undefined ? olds[j] : undefined;
    const f = fields(o, n);
    const kind: LineChange['kind'] = o === undefined ? 'added' : f.some((x) => x.changed) ? 'changed' : 'same';
    return { line: key(n), kind, fields: f };
  });
  olds.forEach((o, j) => {
    if (!taken.has(j)) lines.push({ line: key(o), kind: 'removed', fields: fields(o, undefined) });
  });
  return {
    lines,
    changed: { before: isObj(before) ? str(before.changed) : null, after: isObj(after) ? str(after.changed) : '' },
  };
}
