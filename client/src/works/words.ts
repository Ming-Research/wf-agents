// The works page's words for what the interface gives as data: what a work does now, what it waits for, why a run
// ran, why a return was sent back, what a call is, who wrote a version, and what was taken. Each is worded here once,
// and every part of the page that shows one takes its words from here.

import type {
  Ask,
  Call,
  CallRow,
  Cause,
  CellState,
  Given,
  Limit,
  Now,
  Person,
  Run,
  Spent,
  Summary,
  Unmet,
  Waited,
  Writer,
} from '../works-api';

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const two = (n: number) => String(n).padStart(2, '0');

/// A time as the page shows it, in the browser's time zone: its day and its minute, `11-09 14:02`.
export function clock(ms: number): string {
  const d = new Date(ms);
  return `${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/// A time in full, for the pointer's tip: `2026-11-09 14:02:31`.
export function fullTime(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${clock(ms)}:${two(d.getSeconds())}`;
}

/// The day of a time, `11-09`, and its minute, `14:02`, apart: the history's rows give the day only where it changes.
export function dayOf(ms: number): string {
  return clock(ms).slice(0, 5);
}

export function minuteOf(ms: number): string {
  return clock(ms).slice(6);
}

/// How long something took, in its two largest units: `45s`, `4m 12s`, `2h 5m`, `3d 4h`.
export function duration(ms: number): string {
  const n = Math.max(0, Math.round(ms / SECOND)) * SECOND;
  if (n < MINUTE) return `${n / SECOND}s`;
  if (n < HOUR) return `${Math.floor(n / MINUTE)}m ${Math.floor((n % MINUTE) / SECOND)}s`;
  if (n < DAY) return `${Math.floor(n / HOUR)}h ${Math.floor((n % HOUR) / MINUTE)}m`;
  return `${Math.floor(n / DAY)}d ${Math.floor((n % DAY) / HOUR)}h`;
}

/// Millionths of a dollar in dollars and cents, and what is less than a cent as that.
export function dollars(usdMicros: number): string {
  if (usdMicros > 0 && usdMicros < 10_000) return '<$0.01';
  return `$${(usdMicros / 1_000_000).toFixed(2)}`;
}

/// A count of tokens, short: `812`, `14.4k`, `1.2M`.
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/// Every token a round took: what it was given, what it read from the cache, and what it gave back.
export function allTokens(spent: Spent): number {
  return spent.inputTokens + spent.cachedTokens + spent.outputTokens;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/// What was taken, in one line: `$0.42 · 14k tokens · 4m 12s · 7 rounds`.
export function spentWords(spent: Spent): string {
  return [
    dollars(spent.usdMicros),
    `${tokens(allTokens(spent))} tokens`,
    duration(spent.workingMs),
    plural(spent.rounds, 'round'),
  ].join(' · ');
}

/// The tokens apart, for a panel: `12k in, 8.1k from the cache, 2.1k out`.
export function tokensWords(spent: Spent): string {
  return `${tokens(spent.inputTokens)} in, ${tokens(spent.cachedTokens)} from the cache, ${tokens(spent.outputTokens)} out`;
}

export function personWords(person: Person): string {
  return `the ${person}`;
}

/// A limit a run reached.
export function limitWords(limit: Limit): string {
  return limit.of === 'returns' ? `${plural(limit.sentBack, 'return')} sent back` : `${plural(limit.made, 'call')} made`;
}

/// What a call is, in a line.
export function callWords(call: Call): string {
  switch (call.kind) {
    case 'tool':
      return `the tool ${call.name}`;
    case 'question':
      return `a question to ${personWords(call.who)}`;
    case 'atLimit':
      return `the owner letting the run go on at its limit (${limitWords(call.limit)})`;
  }
}

/// What a round asked for.
export function askWords(ask: Ask): string {
  return ask.kind === 'tool' ? `the tool ${ask.name}` : `a question to ${personWords(ask.who)}`;
}

/// Whether the owner is the one a work waits for, on this.
export function waitsForOwner(waited: Waited): boolean {
  switch (waited.kind) {
    case 'call':
      return waited.who === 'owner' || waited.what.kind === 'atLimit';
    case 'precondition':
      return waited.who === 'owner';
    case 'limit':
      return true;
  }
}

/// What a work waits for, in words; a question's own words are shown beside them, in full.
export function waitedWords(waited: Waited): string {
  switch (waited.kind) {
    case 'call': {
      const what = waited.what;
      if (what.kind === 'question') return `${personWords(what.who)}'s answer to a question`;
      if (what.kind === 'atLimit') return `the owner to let the run go on: it reached its limit, ${limitWords(what.limit)}`;
      return `the result of the tool ${what.name}${waited.who ? ` from ${personWords(waited.who)}` : ''}`;
    }
    case 'precondition':
      return `${waited.words}, which ${personWords(waited.who)} can make hold`;
    case 'limit':
      return `the owner to let the cell go on: ${
        waited.runs === 1 ? 'its last run was a way round' : `its last ${waited.runs} runs in a row were each a way round`
      }`;
  }
}

/// What a work does now, in a line.
export function nowWords(now: Now): string {
  switch (now.kind) {
    case 'fault':
      return `Fault: ${now.words}`;
    case 'merged':
      return 'Merged';
    case 'cancelled':
      return 'Cancelled';
    case 'round':
      return `Running ${now.cell}, round ${now.round} of run ${now.run}, since ${clock(now.since)}`;
    case 'paused':
      return 'Paused: no round begins until it is let on';
    case 'waits':
      return `Waits at ${now.at} for ${now.on.map(waitedWords).join('; ')}`;
    case 'next':
      return `About to go on at ${now.cell}`;
  }
}

/// The word of what a work does now, for its tag, and the tone the tag takes.
export type Tone = 'running' | 'owner' | 'quiet' | 'done' | 'failed';

export function nowTone(now: Now): Tone {
  switch (now.kind) {
    case 'fault':
      return 'failed';
    case 'merged':
      return 'done';
    case 'round':
      return 'running';
    case 'waits':
      return now.on.some(waitsForOwner) ? 'owner' : 'quiet';
    case 'cancelled':
    case 'paused':
    case 'next':
      return 'quiet';
  }
}

/// Why a cell ran, after the block's name where the cause has one.
export function causeRest(cause: Cause): string {
  if (cause.kind === 'noRecord') return 'it had no record';
  const since = cause.was === null ? ', which its record knew no version of' : ` since @${cause.was}`;
  return `changed in meaning${since}${cause.behind ? ', by a cell after it: a way round' : ''}`;
}

export function causeWords(cause: Cause): string {
  return cause.kind === 'noRecord' ? causeRest(cause) : `${cause.block} ${causeRest(cause)}`;
}

/// A run is a way round when each of its reasons is a block whose meaning a cell after it last changed.
export function isWayRound(run: Pick<Run, 'because'>): boolean {
  return run.because.length > 0 && run.because.every((c) => c.kind === 'changed' && c.behind);
}

/// Why what a round handed in was sent back.
export function unmetWords(unmet: Unmet): string {
  switch (unmet.kind) {
    case 'asksNothing':
      return 'it asked for calls and named none';
    case 'notAnOutput':
      return `it handed in ${unmet.block}, which the cell may not write`;
    case 'writtenTwice':
      return `it handed in ${unmet.block} twice`;
    case 'noValue':
      return `it handed in ${unmet.block}, a choice, with no value`;
    case 'notAValue':
      return `it handed in ${unmet.block} as ${unmet.value}, which is not one of its values`;
    case 'notAChoice':
      return `it handed in ${unmet.block} with a value, but it is not a choice`;
  }
}

/// What a round was given.
export function givenWords(given: Given): string {
  switch (given.kind) {
    case 'task':
      return 'its task';
    case 'results':
      return `the results of ${plural(given.calls.length, 'call')}`;
    case 'sentBack':
      return `its return sent back: ${given.unmet.map(unmetWords).join('; ')}`;
  }
}

/// Who wrote a version.
export function writerWords(writer: Writer): string {
  switch (writer.kind) {
    case 'person':
      return 'a person';
    case 'outside':
      return 'the outside';
    case 'run':
      return `run ${writer.run} of ${writer.cell}`;
  }
}

export const STATE_WORDS: Record<CellState, string> = {
  running: 'running',
  resting: 'at rest',
  skipped: 'skipped',
  notYet: 'not run yet',
  again: 'runs again',
  done: 'done',
};

/// How a run stands: it finished, it was abandoned, a round of it is under way, or it is at rest between rounds.
export type RunState = 'finished' | 'abandoned' | 'running' | 'resting';

export function runState(run: Pick<Run, 'end' | 'rounds'>): RunState {
  if (run.end) return run.end.kind;
  const last = run.rounds[run.rounds.length - 1];
  return last && last.ended === null ? 'running' : 'resting';
}

export const RUN_WORDS: Record<RunState, string> = {
  finished: 'finished',
  abandoned: 'abandoned',
  running: 'under way',
  resting: 'at rest',
};

/// How long a run took from its start: to its end, or until now while it is open.
export function runTook(run: Pick<Run, 'time' | 'end'>, now: number): number {
  return (run.end?.time ?? now) - run.time;
}

/// A call's state in words: when it was made and, once it has one, when its result came.
export function callState(row: CallRow): string {
  return row.done === null ? `made ${clock(row.made)}, no result yet` : `made ${clock(row.made)}, done in ${duration(row.done - row.made)}`;
}

/// Whether a work has ended: merged, or its item cancelled.
export function hasEnded(summary: Pick<Summary, 'now'>): boolean {
  return summary.now.kind === 'merged' || summary.now.kind === 'cancelled';
}
