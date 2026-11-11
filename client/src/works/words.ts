// The works page's words for what the interface gives as data: what a work does now, what it waits for, why a run
// ran, why a return was sent back, what a call is, who wrote a version, and what was taken. Each is worded here once,
// and every part of the page that shows one takes its words from here.

import type {
  Ask,
  Call,
  CallRow,
  Cause,
  CellState,
  Failure,
  Given,
  Limit,
  Maker,
  Now,
  Outage,
  Pauser,
  Person,
  Run,
  SessionWhy,
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

/// Why a line began a new session, as a clause: `a new one, its context having been 612k tokens, past the limit of 500k`.
export function sessionWords(why: SessionWhy): string {
  switch (why.kind) {
    case 'limit':
      return `a new one, its context having been ${tokens(why.context)} tokens, past the limit of ${tokens(why.limit)}`;
    case 'model':
      return `a new one, its rounds running on another model than ${why.was}`;
    case 'account':
      return 'a new one, its rounds running under another account';
    case 'owner':
      return 'a new one, as the owner asked';
    case 'flagged':
      return "a new one, the service's safety check having flagged its last round";
    case 'lost':
      return "begun again, the agents' program having lost it";
  }
}

export function personWords(person: Person): string {
  return `the ${person}`;
}

/// Who can make a precondition hold.
export function makerWords(maker: Maker): string {
  switch (maker.kind) {
    case 'owner':
      return 'the owner';
    case 'outside':
      return `the outside (${maker.source})`;
    case 'cell':
      return `cell ${maker.cell}`;
  }
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
    case 'wait':
      return `the engine's wait until ${clock(call.until)}, before a round that failed in passing is tried again (${call.failed} in a row)`;
  }
}

/// The kind of cause of a round that failed, as a clause.
export function failureWords(failure: Failure): string {
  switch (failure) {
    case 'passing':
      return 'in passing';
    case 'unread':
      return 'what came back could not be read';
    case 'flagged':
      return "the service's safety check flagged it";
    case 'usageLimit':
      return "the account's usage limit was reached";
    case 'cannotRun':
      return "the agents' command line cannot run";
    case 'offline':
      return 'the network cannot be reached';
    case 'promptTooLong':
      return 'the prompt is too long for the model';
    case 'other':
      return 'a cause no kind tells apart';
  }
}

/// A wait every work shares, in a line: its cause, when it is over, and the round that met it.
export function outageWords(outage: Outage): string {
  const until = outage.until;
  const when =
    until.kind === 'at'
      ? `until ${clock(until.at)}`
      : until.kind === 'tried'
        ? 'one round tries now'
        : until.look
          ? `one round tries once the network is reached, looked at from ${clock(until.at)}, at the latest at ${clock(until.latest)}`
          : `one round tries at ${clock(until.at)}`;
  const mend = outage.failure === 'cannotRun' ? ': sign the command line in again or install it' : '';
  return `${failureWords(outage.failure)}${mend}; no agent's round of any work begins, ${when} (work ${outage.work}, round ${outage.round} of run ${outage.run}: ${outage.why})`;
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
      return waited.by.some((m) => m.kind === 'owner');
    case 'limit':
      return true;
    case 'slot':
      return false;
    case 'agents':
      return waited.outage.failure === 'cannotRun';
  }
}

/// What a work waits for, in words; a question's own words are shown beside them, in full.
export function waitedWords(waited: Waited): string {
  switch (waited.kind) {
    case 'call': {
      const what = waited.what;
      if (what.kind === 'question') return `${personWords(what.who)}'s answer to a question`;
      if (what.kind === 'atLimit') return `the owner to let the run go on: it reached its limit, ${limitWords(what.limit)}`;
      if (what.kind === 'wait') return callWords(what);
      return `the result of the tool ${what.name}${waited.who ? ` from ${personWords(waited.who)}` : ''}`;
    }
    case 'precondition':
      return `${waited.words}, which ${waited.by.map(makerWords).join(' or ')} can make hold`;
    case 'limit':
      return `the owner to let the cell go on: ${
        waited.runs === 1 ? 'its last run was a way round' : `its last ${waited.runs} runs in a row were each a way round`
      }`;
    case 'slot':
      return 'a free slot, which the engine gives in its order';
    case 'agents':
      return `the agents' rounds to begin again: ${outageWords(waited.outage)}`;
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
      return now.by === 'engine'
        ? `Paused by the engine: ${now.why ?? 'it gave no reason'}`
        : 'Paused: only a note to a cell is answered until it is let on';
    case 'everything':
      return 'Paused with every work: only a note to a cell is answered until everything is let on';
    case 'waits':
      return `Waits at ${now.at} for ${now.on.map(waitedWords).join('; ')}`;
    case 'next':
      return `About to go on at ${now.cell}`;
  }
}

/// The word of what a work does now, for its tag, and the tone the tag takes.
export type Tone = 'running' | 'owner' | 'quiet' | 'done' | 'failed';

/// A work the engine paused, as for a round that failed, is in the tone of what failed, as a fault is: the owner looks
/// before it goes on.
export function nowTone(now: Now): Tone {
  switch (now.kind) {
    case 'fault':
      return 'failed';
    case 'paused':
      return now.by === 'engine' ? 'failed' : 'quiet';
    case 'merged':
      return 'done';
    case 'round':
      return 'running';
    case 'waits':
      return now.on.some(waitsForOwner) ? 'owner' : 'quiet';
    case 'cancelled':
    case 'everything':
    case 'next':
      return 'quiet';
  }
}

/// Why a cell ran or would run, after the block's name where the cause has one. A change a cell after it made is said
/// to be one; the run is a way round only when every one of its causes is (`wayRound`).
export function causeRest(cause: Cause, wayRound = false): string {
  if (cause.kind === 'noRecord') return 'the cell had no record';
  const since = cause.was === null ? ', which its record knew no version of' : ` since @${cause.was}`;
  const behind = cause.behind ? `, by a cell after it${wayRound ? ': a way round' : ''}` : '';
  return `changed in meaning${since}${behind}`;
}

export function causeWords(cause: Cause, wayRound = false): string {
  const rest = causeRest(cause, wayRound);
  return cause.kind === 'noRecord' ? rest : `${cause.block} ${rest}`;
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
    case 'notHandedIn':
      return `it did not hand in ${unmet.block}, which the cell must hand in every time`;
    case 'unreadable':
      return `what it returned could not be read: ${unmet.why}`;
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
  failed: 'failed',
  skipped: 'skipped',
  notYet: 'not run yet',
  again: 'runs again',
  done: 'done',
};

/// How a run stands: it finished, it was abandoned, a round of it is under way, its last round failed, or it is at
/// rest between rounds.
export type RunState = 'finished' | 'abandoned' | 'running' | 'failed' | 'resting';

export function runState(run: Pick<Run, 'end' | 'rounds'>): RunState {
  if (run.end) return run.end.kind;
  const last = run.rounds[run.rounds.length - 1];
  if (last && last.ended === null) return 'running';
  return last?.back?.kind === 'failed' ? 'failed' : 'resting';
}

export const RUN_WORDS: Record<RunState, string> = {
  finished: 'finished',
  abandoned: 'abandoned',
  running: 'under way',
  failed: 'round failed',
  resting: 'at rest',
};

/// Whether a run is open: neither finished nor abandoned.
export const isOpen = (state: RunState): boolean => state !== 'finished' && state !== 'abandoned';

/// Who paused a work, and why, in a line.
export function pauseWords(by: Pauser, why: string | null): string {
  return by === 'engine' ? `the engine paused the work: ${why ?? 'it gave no reason'}` : 'the owner paused the work';
}

/// How long a run took from its start: to its end, or until now while it is open.
export function runTook(run: Pick<Run, 'time' | 'end'>, now: number): number {
  return (run.end?.time ?? now) - run.time;
}

/// A call's state in words: when it was made and, once it has one, when its result came; a call with no result of a
/// run that was abandoned was withdrawn with it.
export function callState(row: CallRow, abandoned = false): string {
  const made = `made ${clock(row.made)}`;
  if (row.done !== null) return `${made}, done in ${duration(row.done - row.made)}`;
  return abandoned ? `${made}, withdrawn with its run` : `${made}, no result yet`;
}

/// Whether a work has ended: merged, or its item cancelled.
export function hasEnded(summary: Pick<Summary, 'now'>): boolean {
  return summary.now.kind === 'merged' || summary.now.kind === 'cancelled';
}
