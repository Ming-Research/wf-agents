// The works page's words for what the interface gives as data: what a work does now, what it waits for, why a run
// ran, what a call is, who wrote a version, and what was taken; and the pieces a line of words is made of. Each is
// worded here once, and every part of the page that shows one takes its words from here; the
// names of the marks are the table's (`marks.tsx`).

import type {
  Call,
  CallRow,
  Cause,
  Failure,
  Limit,
  Maker,
  Outage,
  Person,
  RanOn,
  Run,
  SessionWhy,
  Spent,
  Summary,
  Waited,
  Writer,
} from '../works-api';
import type { OrderBy, Way } from './placing';
import { waitsOf } from './waiting';

/// A piece of a line of words: the page's own words, a name or a value as the files write it, drawn in monospace, or a
/// link out of the page, by its words.
export type Part = string | { name: string } | { link: string; href: string };

/// A line of pieces as one text, for the pointer's tip.
export const partsText = (parts: Part[]): string =>
  parts.map((p) => (typeof p === 'string' ? p : 'name' in p ? p.name : p.link)).join('');

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

/// The day of a time, `11-09`, and its minute, `14:02`, apart: a check's time gives the day only where it is not the day
/// the wait began.
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

/// A count in full, its thousands apart: `12,345`.
export function count(n: number): string {
  return n.toLocaleString('en');
}

/// Lines added and removed, as a diff counts them: `+210`, `−40`.
export const linesAdded = (n: number): string => `+${count(n)}`;
export const linesRemoved = (n: number): string => `−${count(n)}`;

/// What share of a whole a part is, in whole percent: `89%`; a dash where the whole is nothing.
export function share(part: number, whole: number): string {
  return whole > 0 ? `${Math.round((100 * part) / whole)}%` : '–';
}

/// A key of what was taken, or what the page works out from them, said plainly, with its value.
export type Key = [string, string];

/// The key the page shows a cost under: the records keep it in millionths of a dollar, and the page says it in dollars.
export const DOLLARS = 'usd_micros in dollars';

/// The key the tokens sent and not read from the cache are shown under where the records do not tell those sent fresh
/// from those written to the cache (the rounds, pings and forks kept before they were told apart).
export const UNSPLIT = 'input_or_cache_written_tokens';

/// Every token that was sent: sent fresh, written to the cache, not told which of the two, and read from the cache.
export function sentTokens(spent: Spent): number {
  return spent.inputTokens + spent.cacheWrittenTokens + spent.unsplitTokens + spent.cachedTokens;
}

/// The share of what was sent that was read from the cache, as the page works it out from the records' keys: the key
/// of its working, which names the kinds of token it adds up.
export function cachedShareKey(spent: Spent): string {
  const sent = ['input_tokens', 'cache_written_tokens', ...(spent.unsplitTokens > 0 ? [UNSPLIT] : []), 'cached_tokens'];
  return `cached_tokens / (${sent.join(' + ')})`;
}

/// What a round, or rounds together, took, by each key the records keep it under, with its value, and the share read
/// from the cache.
export function spentKeys(spent: Spent): Key[] {
  return [
    [DOLLARS, dollars(spent.usdMicros)],
    ['input_tokens', count(spent.inputTokens)],
    ['cache_written_tokens', count(spent.cacheWrittenTokens)],
    ...(spent.unsplitTokens > 0 ? [[UNSPLIT, count(spent.unsplitTokens)] as Key] : []),
    ['cached_tokens', count(spent.cachedTokens)],
    ['output_tokens', count(spent.outputTokens)],
    [cachedShareKey(spent), share(spent.cachedTokens, sentTokens(spent))],
  ];
}

/// Keys with their values in a line, for the pointer's tip: `rounds 3, calls 2`.
export function keysLine(keys: Key[]): string {
  return keys.map(([key, value]) => `${key} ${value}`).join(', ');
}

/// Every token a round took: what it was given, fresh or written to the cache, what it read from the cache, and what it
/// gave back.
export function allTokens(spent: Spent): number {
  return sentTokens(spent) + spent.outputTokens;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/// What was taken, in one line: `$0.42 · 14k tokens`; how long it took is said where it began.
export function spentWords(spent: Spent): string {
  return [dollars(spent.usdMicros), `${tokens(allTokens(spent))} tokens`].join(' · ');
}

/// The tokens apart, for a panel: `12k in, 3k written to the cache, 8.1k from the cache, 2.1k out`; with, where the
/// records do not tell some of those sent and not read from the cache fresh from written, `4k in or written to the
/// cache, not told apart` after the written ones.
export function tokensWords(spent: Spent): string {
  return [
    `${tokens(spent.inputTokens)} in`,
    `${tokens(spent.cacheWrittenTokens)} written to the cache`,
    ...(spent.unsplitTokens > 0 ? [`${tokens(spent.unsplitTokens)} in or written to the cache, not told apart`] : []),
    `${tokens(spent.cachedTokens)} from the cache`,
    `${tokens(spent.outputTokens)} out`,
  ].join(', ');
}

/// Why a line began a new session, as a clause: `a new one, its context having been 612k tokens, past the limit of 500k`.
export function sessionWords(why: SessionWhy): string {
  switch (why.kind) {
    case 'limit':
      return `a new one, its context having been ${tokens(why.context)} tokens, past the limit of ${tokens(why.limit)}`;
    case 'model':
      return `a new one, its rounds running on another model than ${why.was}`;
    case 'changed': {
      const changed = 'a new one, the rules or the form of return its agents are given having changed since it began';
      return why.context === null
        ? changed
        : `${changed}, the session it left holding ${tokens(why.context)} tokens of context`;
    }
    case 'account':
      return 'a new one, its rounds running under another account';
    case 'owner':
      return 'a new one, as the owner asked';
    case 'flagged':
      return "a new one, the service's safety check having flagged its last round";
    case 'guarded':
      return 'a new one, the run guard having stopped its last round';
    case 'tooLong':
      return "a new one, its last round's prompt having been too long for the model";
    case 'lost':
      return "begun again, the agents' program having lost it";
    case 'compacted':
      return 'it holds only a summary of what it read';
  }
}

export function personWords(person: Person): string {
  return `the ${person}`;
}

/// Who can make a precondition hold, as the engine says it from what the flow declares of each block.
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
    case 'guarded':
      return "the run guard stopped it, its context growing past what its cell's history explains";
    case 'usageLimit':
      return "the account's usage limit was reached";
    case 'cannotRun':
      return "the agents' command line cannot run";
    case 'offline':
      return 'the network cannot be reached';
    case 'unavailable':
      return "the agents' service is down, overloaded or not answering";
    case 'promptTooLong':
      return 'the prompt is too long for the model';
    case 'noModel':
      return "the service has no model of the name the round's level gives";
    case 'other':
      return 'a cause no kind tells apart';
  }
}

/// What is down in a wait every work shares: as the latest check of the outside found it while the wait holds, or else
/// as the round that met it did.
export function outageDown(outage: Outage): { failure: Failure; why: string } {
  const found = outage.checked?.found;
  return outage.holds && found?.kind === 'down' ? found : { failure: outage.failure, why: outage.why };
}

/// A text with its first letter a capital, to begin a sentence.
const capital = (text: string): string => text.charAt(0).toUpperCase() + text.slice(1);

/// What a wait every work shares holds and until when, a sentence for each kind of cause; a usage limit ends as well
/// where the engine may move to another account.
export function heldWords(service: Outage['service'], failure: Failure, mayMove = false): string {
  if (service === 'host') return "No act of the engine's begins until it is reached again.";
  switch (failure) {
    case 'cannotRun':
      return "No agent's round begins until it runs again.";
    case 'usageLimit':
      return mayMove
        ? "No agent's round begins until the limit resets or the engine moves to another account."
        : "No agent's round begins until the limit resets.";
    case 'offline':
      return "No agent's round begins until the service is reached again.";
    case 'unavailable':
      return "No agent's round begins until the service answers again.";
    default:
      return "No agent's round begins until the cause is gone.";
  }
}

/// A wait every work shares, in sentences: what is down and since when; where the latest check of the outside found it
/// down, what it found, in the engine's words (never the error of the round that met it); and what it holds until it
/// runs again. None where it holds no work: nothing is then for the owner to know or do. A name or a value in backticks
/// in what was found is marked so (`Coded` draws it).
export function outageWords(outage: Outage): string | null {
  if (!outage.holds) return null;
  if (outage.first != null) {
    const what = outage.service === 'host' ? 'act' : 'round';
    return `${capital(failureWords(outage.failure))} since ${clock(outage.since)}. It is well again: ${workName(outage.first)}'s ${what} goes first, and the rest begin once it comes back.`;
  }
  const found = outage.checked?.found;
  const down = outageDown(outage);
  // The check's time, without its day where that is the day the wait began.
  const at = (ms: number) => (dayOf(ms) === dayOf(outage.since) ? minuteOf(ms) : clock(ms));
  const why = outage.checked && found?.kind === 'down' ? ` At ${at(outage.checked.at)} the check found: ${sentence(found.why)}` : '';
  return `${capital(failureWords(down.failure))} since ${clock(outage.since)}.${why} ${heldWords(outage.service, down.failure, outage.mayMove)}`;
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
    case 'calls':
      return `the owner to let the cell go on: ${
        waited.runs === 1
          ? 'its last run asked for calls and dealt with nothing'
          : `its last ${waited.runs} runs in a row each asked for calls and dealt with nothing`
      }`;
    case 'slot':
      return 'a free slot, which the engine gives in its order';
    case 'agents': {
      const outage = waited.outage;
      if (outage.first != null) {
        const what = outage.service === 'host' ? 'act' : 'round';
        return `${workName(outage.first)}'s ${what} to come back: it is well again, and that ${what} goes first`;
      }
      return `the agents' rounds to begin again, as ${failureWords(outageDown(outage).failure)}`;
    }
    case 'stopping':
      return `the engine to start again, as it was asked to stop at ${clock(waited.since)}`;
    case 'answer':
      return `the owner's answer to the question ${waited.question}`;
    case 'outside':
      return `${waited.source} to move, before ${waited.cell} runs again: ${waited.why}`;
    case 'turn':
      return `the project's turn to check and merge, which ${workName(waited.holder)} has`;
    case 'items':
      return itemsWords(waited.items);
  }
}

/// A work's number as the engine writes it in words, everywhere: `W5`.
export const workName = (number: number): string => `W${number}`;

/// An item's number as the engine writes it in words, everywhere: `#5`.
export const itemName = (number: number): string => `#${number}`;

/// Items by their numbers, as a line names them: `#5`, `#5 and #7`, `#5, #6 and #7`.
export function itemsWords(items: number[]): string {
  const named = items.map(itemName);
  return named.length < 2 ? named.join('') : `${named.slice(0, -1).join(', ')} and ${named[named.length - 1]}`;
}

/// The words of a work's state, the one catalogue every view names one from: the item's state line in the console, a
/// row of the Works list, the System view's tiles, a stretch of a work's bar and the browser's notifications. A work
/// that waits for anyone but the owner is waiting for them, by name (`waitingFor`).
export const STATE = {
  owner: 'Waiting for you',
  halted: 'Halted',
  paused: 'Paused',
  running: 'Running',
  merged: 'Merged',
  cancelled: 'Cancelled',
  /// Waiting for the items the work's item depends on to merge.
  items: 'Waiting for the items it depends on',
  /// Waiting, where nothing the engine gives names who gives it.
  waiting: 'Waiting',
} as const;

/// What the history of a work on no version kept says over it: why its cells have no titles or lines (decision
/// `a-works-past-is-drawn-from-its-outline`).
export const NOT_KEPT = 'The flow this work ran on was not kept: its cells are named by the ids its records name.';

/// A work that waits for whoever else gives what it waits for, by name.
export const waitingFor = (who: string): string => `Waiting for ${who}`;

/// What the button of a card that ends a wait says, and what it does on pointing at it.
export const ENDS_WORDS = {
  goOn: { label: 'Let it go on', title: 'Give the run its next round past its limit' },
  letGo: { label: 'Let it go on', title: 'Run the cell again past its limit' },
  resume: { label: 'Resume', title: "Lift the engine's pause and let the work go on" },
} as const;

/// What the card of a block the owner may write says on its button, which writes the value picked, and on pointing at
/// it, before a value is picked and once one is.
export const WRITE_WORDS = {
  label: 'Confirm',
  none: 'Pick a value to write',
  writes: (block: string, value: string): string => `Write ${block} = ${value}`,
} as const;

/// What a pointer to a question does, said on pointing at it: the pointer stands in a text, and the question's card
/// elsewhere in the item's window.
export const SHOW_QUESTION = 'Show the question this points to';

/// The mark on the option the owner took in a question answered, said on pointing at it.
export const TAKEN = 'The option you took';

/// What the switch that pauses a work is: the owner's own pause of it, which the engine's pause is not.
export const OWN_PAUSE = {
  label: 'Your own pause of the work',
  on: { name: 'Start', title: 'Lift your pause and let the work go on' },
  paused: { name: 'Pause', title: 'Nothing of the work goes on by itself; what you write to it is still answered' },
} as const;

/// What a count of the things that wait for the owner counts, after the number.
export const waitCounted = (n: number): string => `${n === 1 ? 'thing waits' : 'things wait'} for you`;

/// How many things wait for the owner, the one count, said one way wherever it is said.
export const waitCount = (n: number): string => `${n} ${waitCounted(n)}`;

/// How many questions the owner may answer ahead, which hold no work yet, said one way wherever it is said.
export const aheadCount = (n: number): string =>
  `${n} ${n === 1 ? 'question' : 'questions'} you may answer ahead`;

/// Who gives what a work waits for that is not the owner's: the engine for a call it makes, a slot and the wait every
/// work shares, the source outside the work, or else the cells, for a
/// precondition only they make hold, the source a work waits on, and the work that holds the project's turn. None for
/// what waits on the owner, which the engine's list says.
function whoGives(w: Waited): string | null {
  switch (w.kind) {
    case 'call':
      return w.who === null ? 'the engine' : null;
    case 'precondition': {
      const sources = w.by.flatMap((m) => (m.kind === 'outside' ? [m.source] : []));
      const cells = w.by.flatMap((m) => (m.kind === 'cell' ? [m.cell] : []));
      const who = sources.length > 0 ? sources : cells;
      return who.length > 0 ? who.join(' and ') : null;
    }
    case 'slot':
    case 'agents':
    case 'stopping':
      return 'the engine';
    case 'items':
      return 'the items it depends on';
    case 'outside':
      return w.source;
    case 'turn':
      return workName(w.holder);
    case 'limit':
    case 'calls':
    case 'answer':
      return null;
  }
}

/// The tone a work's state takes: red for a work that waits for the owner and for one halted, a stop either way; the
/// yellow of a call for one that waits for the calls of tools the engine carries out.
export type Tone = 'running' | 'calling' | 'owner' | 'failed' | 'quiet' | 'done';

/// A call of a tool a work waits for the result of, which the engine carries out: the tool, by its name; the link of
/// the CI run it runs as, where the engine gives one; and when it was made, none where the engine does not say.
export interface CallWaited {
  tool: string;
  ci: string | null;
  made: number | null;
}

/// The calls of tools a work waits at its cell for the results of, which the engine carries out, in the order it waits
/// for them; none where it waits for none. Its cell is then the one the engine marks as carrying out a call
/// (`calling`).
export function callsWaited(s: Pick<Summary, 'now'>): CallWaited[] {
  const now = s.now;
  if (now.kind !== 'waits') return [];
  return now.on.flatMap((w) =>
    engineToolCall(w) ? [{ tool: w.what.name, ci: w.at ?? null, made: w.made ? w.made : null }] : [],
  );
}

/// Whether a wait is for the result of a tool's call the engine carries out.
const engineToolCall = (w: Waited): w is Extract<Waited, { kind: 'call' }> & { what: { kind: 'tool' } } =>
  w.kind === 'call' && w.who === null && w.what.kind === 'tool';

/// What a work that waits for calls of tools waits for, the word of its state, in pieces: each tool by its name, and
/// where it runs: in CI, a link to its run, where the engine gives the run's link, else by the engine. `Waiting for
/// run_tests in CI`.
export function callWaitParts(calls: CallWaited[]): Part[] {
  return [
    'Waiting for ',
    ...calls.flatMap((c, i): Part[] => [
      ...(i === 0 ? [] : [' and ']),
      { name: c.tool },
      ...(c.ci !== null ? [' in ', { link: 'CI', href: c.ci }] : [' by the engine']),
    ]),
  ];
}

/// How long a work has waited for the calls of tools it waits for, from the earliest made, at `time`; none where it
/// waits for none, or the engine does not say when they were made.
export function callsTook(calls: CallWaited[], time: number): string | null {
  const made = calls.flatMap((c) => (c.made === null ? [] : [c.made]));
  return made.length === 0 ? null : duration(time - Math.min(...made));
}

/// The word of a work's state in pieces, as a line that can hold a link draws it: the calls of tools it waits for
/// with where they run (`callWaitParts`), else the word alone.
export function stateParts(s: Pick<Summary, 'now' | 'cells' | 'waits'>): Part[] {
  const state = stateOf(s);
  return state.tone === 'calling' ? callWaitParts(callsWaited(s)) : [state.word];
}

/// What a work's state line says: the tone of its dot, the word of its state from the catalogue, and the cell it
/// stands at where the engine's state names one.
export interface WorkState {
  tone: Tone;
  word: string;
  cell: string | null;
}

/// The state of a work. Whatever the engine's list of what waits on the owner holds waits for you; a fault and a stalled
/// work are halted; the owner's own pause, and the owner's pause of everything, are a pause; a round under way, an act
/// of the engine's own among them, is running; the calls of tools the engine carries out for a cell are waited for,
/// each by its tool and where it runs, in the yellow of a call, and are neither running nor done; anything else waits
/// for whoever gives it.
export function stateOf(s: Pick<Summary, 'now' | 'cells' | 'waits'>): WorkState {
  const now = s.now;
  const at =
    now.kind === 'waits' || now.kind === 'stalled' || now.kind === 'paused' || now.kind === 'everything'
      ? now.at
      : now.kind === 'round' || now.kind === 'next'
        ? now.cell
        : null;
  if (waitsOf(s).length > 0) return { tone: 'owner', word: STATE.owner, cell: at };
  switch (now.kind) {
    case 'round':
    case 'next':
      return { tone: 'running', word: STATE.running, cell: at };
    case 'waits': {
      const calls = callsWaited(s);
      if (calls.length > 0) return { tone: 'calling', word: partsText(callWaitParts(calls)), cell: at };
      if (now.on.length > 0 && now.on.every((w) => w.kind === 'items')) return { tone: 'quiet', word: STATE.items, cell: at };
      const who = [...new Set(now.on.flatMap((w) => whoGives(w) ?? []))];
      return { tone: 'quiet', word: who.length > 0 ? waitingFor(who.join(' and ')) : STATE.waiting, cell: at };
    }
    case 'paused':
    case 'everything':
      return { tone: 'quiet', word: STATE.paused, cell: at };
    case 'fault':
      return { tone: 'failed', word: STATE.halted, cell: null };
    case 'stalled':
      return { tone: 'failed', word: STATE.halted, cell: at };
    case 'merged':
      return { tone: 'done', word: STATE.merged, cell: null };
    case 'cancelled':
      return { tone: 'quiet', word: STATE.cancelled, cell: null };
  }
}

/// The cell a work is stopped at, for the owner or by the engine, whose mark is drawn as `stopped` wherever the work's
/// cells are drawn (state-marks.ts); none while it is not so stopped. Where the engine names no cell, the cell with a
/// run open.
export function stoppedAt(s: Pick<Summary, 'now' | 'cells' | 'waits'>): string | null {
  const { tone } = stateOf(s);
  if (tone !== 'owner' && tone !== 'failed') return null;
  const now = s.now;
  if (now.kind === 'round' || now.kind === 'next') return now.cell;
  if (now.kind === 'waits' || now.kind === 'stalled' || now.kind === 'paused' || now.kind === 'everything') {
    return now.at;
  }
  return s.cells.find((c) => c.open !== null)?.cell ?? null;
}

/// The first line of a text, for a line of a list or a notification.
export const firstLine = (text: string): string => text.trim().split('\n')[0] ?? '';

/// A text as a sentence: as it is where it ends as a sentence does, in English or in Chinese, else with a full stop; so
/// that reasons said one after another each read as one. The engine's own rule (`flow::agents::sentence`).
export function sentence(text: string): string {
  const t = text.trim();
  return t === '' || /[.?!。？！]$/.test(t) ? t : `${t}.`;
}

/// A reason as a line of words says it, where no name can be set in the monospace: its names and values without the
/// backticks that mark them (a card draws them in the monospace, by `Coded` in `parts.tsx`).
export const plain = (text: string): string => text.replace(/`([^`\n]+)`/g, '$1');

/// What the head of a column of the Works list that orders it says on pointing at it: the order a press gives it, by
/// the way it would run.
export const ORDER_WORDS: Record<OrderBy, Record<Way, string>> = {
  item: { descending: 'Order by item, the newest first', ascending: 'Order by item, the oldest first' },
  title: { ascending: 'Order by title, from A to Z', descending: 'Order by title, from Z to A' },
  state: {
    ascending: 'Order by state: waiting for you, halted, running, waiting, paused, merged, cancelled',
    descending: 'Order by state: cancelled, merged, paused, waiting, running, halted, waiting for you',
  },
  level: {
    descending: 'Order by level, the highest first, those with none last',
    ascending: 'Order by level, the lowest first, those with none last',
  },
  cellLevel: {
    descending: "Order by the cell's level, the highest first, those with none last",
    ascending: "Order by the cell's level, the lowest first, those with none last",
  },
  cost: { descending: 'Order by cost, the dearest first', ascending: 'Order by cost, the cheapest first' },
  time: {
    descending: 'Order by the time its rounds took, the longest first',
    ascending: 'Order by the time its rounds took, the shortest first',
  },
  last: {
    descending: 'Order by when something last happened, the latest first',
    ascending: 'Order by when something last happened, the earliest first',
  },
};

/// What a column of levels holds where the engine gives no level, and why, on pointing at it.
export const NO_LEVEL = {
  shown: '–',
  level: 'The engine cannot say the level its rounds run at',
  cellLevel: 'It stands at no cell an agent runs at, or the engine cannot say its level',
} as const;

/// A work's state in a line: its state's word; how long it has waited, at `time`, for the calls of tools it waits for;
/// and the cell where the state names one; as the Works list's column of states says it, `Paused · R0`, `Merged`,
/// `Waiting for run_tests in CI · 4m 12s · I1`.
export function stateWords(s: Pick<Summary, 'now' | 'cells' | 'waits'>, time = Date.now()): string {
  const { tone, word, cell } = stateOf(s);
  const took = tone === 'calling' ? callsTook(callsWaited(s), time) : null;
  return [word, took, cell].filter((x) => x !== null && x !== '').join(' · ');
}

/// What a work does now, in its view's line: its state's words, and why: for what waits on the owner, the engine's
/// reasons, each a sentence; for a fault or a stall, what is wrong, a stall as the engine says it; for what waits on
/// another, what it waits for, but the calls of tools, which its state's words say.
export function nowWords(s: Pick<Summary, 'now' | 'cells' | 'waits'>, time = Date.now()): string {
  const now = s.now;
  const waits = waitsOf(s);
  const why =
    waits.length > 0
      ? waits.map((w) => sentence(firstLine(w.why))).join(' ')
      : now.kind === 'fault'
        ? now.words
        : now.kind === 'stalled'
          ? now.why
          : now.kind === 'waits'
            ? now.on.filter((w) => !engineToolCall(w)).map(waitedWords).join('; ')
            : null;
  return [stateWords(s, time), why === null ? null : plain(why)].filter((x) => x !== null && x !== '').join(' · ');
}

/// The tone of what a work does now, as its state takes it (`stateOf`).
export const nowTone = (s: Pick<Summary, 'now' | 'cells' | 'waits'>): Tone => stateOf(s).tone;

/// Why a cell ran or would run, after the block's name where the cause has one. A change a cell after it made is said
/// to be one; the run is a way round only when every one of its causes is (`wayRound`).
export function causeRest(cause: Cause, wayRound = false): string {
  if (cause.kind === 'noRecord') return 'it had not run before';
  if (cause.kind === 'results') return `the results of the ${plural(cause.calls.length, 'call')} it asked for came back`;
  const how =
    cause.workedFrom === null ? 'new to it' : `changed since the v${cause.workedFrom} it last worked from`;
  const behind = cause.behind ? `, written by a cell after it${wayRound ? ': a way round' : ''}` : '';
  return `${how}${behind}`;
}

export function causeWords(cause: Cause, wayRound = false): string {
  const rest = causeRest(cause, wayRound);
  return cause.kind === 'changed' ? `${cause.block} ${rest}` : rest;
}

/// A run is a way round when each of its reasons is a block whose meaning a cell after it last changed.
export function isWayRound(run: Pick<Run, 'because'>): boolean {
  return run.because.length > 0 && run.because.every((c) => c.kind === 'changed' && c.behind);
}

/// Who wrote a version.
export function writerWords(writer: Writer): string {
  switch (writer.kind) {
    case 'owner':
      return 'the owner';
    case 'outside':
      return 'the outside';
    case 'run':
      return writer.cell;
  }
}

/// Whether a run is open: neither finished nor abandoned.
export const isOpen = (run: Pick<Run, 'end'>): boolean => run.end === null;

/// How long a run took from its start: to its end, or until now while it is open.
export function runTook(run: Pick<Run, 'time' | 'end'>, now: number): number {
  return (run.end?.time ?? now) - run.time;
}

/// What a run's latest round ran on, as the records of its latest try say; none of a run no agent's round of which
/// kept it, as an engine cell's.
export function ranLast(run: Pick<Run, 'rounds'>): RanOn | null {
  for (const round of [...run.rounds].reverse()) {
    const ran = round.tries[round.tries.length - 1]?.ran;
    if (ran) return ran;
  }
  return null;
}

/// A call's state in words: when it was made and, once it has one, when its result came; a call with no result of a
/// run that was abandoned was withdrawn with it.
export function callState(row: CallRow, abandoned = false): string {
  const made = `made ${clock(row.made)}`;
  if (row.done !== null) return `${made}, done in ${duration(row.done - row.made)}`;
  return abandoned ? `${made}, withdrawn with its run` : `${made}, no result yet`;
}

/// What a cancel does, said before the owner makes one.
export const CANCEL_DOES = 'Nothing of it goes on until its item is reopened, and then it goes on from where it stopped';

/// What the owner is asked for as they cancel a work, which is kept with the cancel.
export const CANCEL_WHY = 'Why it is cancelled, in a sentence, such as the item it was joined into';

/// Whether a work has ended: merged, or its item cancelled.
export function hasEnded(summary: Pick<Summary, 'now'>): boolean {
  return summary.now.kind === 'merged' || summary.now.kind === 'cancelled';
}

