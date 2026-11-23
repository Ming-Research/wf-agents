import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import type { Spent, Summary, Wait } from '../works-api';
import { fixtureItems } from '../works/fixtures';
import { changeables, problem, unquoted } from './SettingsView';
import { fixtureSettings } from './fixtures';
import { STANDING_WORDS, cacheShare, mergeAverages, nextWeekStart, standingOf, summed, weekStarts } from './standing';

const spent = (usdMicros: number, inputTokens = 0, cachedTokens = 0): Spent => ({
  usdMicros,
  inputTokens,
  cachedTokens,
  outputTokens: 0,
  workingMs: 1000,
  rounds: 1,
});

/// A work doing this now, waiting on the owner for these, as the engine's list says.
const work = (now: Summary['now'], waits: Wait[] = []): Summary => ({
  ...fixtureItems().items[0].work,
  now,
  paused: null,
  waits,
});

const wait = (key: string): Wait => ({ key, why: `${key}.`, ends: { kind: 'resume' } });

describe('where a work stands', () => {
  it('names the tiles by the states\' own words', () => {
    expect(STANDING_WORDS.owner.name).toBe('Waiting for you');
    expect(STANDING_WORDS.others.name).toBe('Waiting for others');
    expect(STANDING_WORDS.halted.name).toBe('Halted');
  });

  it('is in one place, what needs the owner first, and nowhere once it has ended', () => {
    expect(standingOf(work({ kind: 'merged' }))).toBeNull();
    expect(standingOf(work({ kind: 'cancelled' }))).toBeNull();
    expect(standingOf(work({ kind: 'fault', words: 'broken' }))).toBe('halted');
    // What the engine's list holds waits for the owner, the engine's pause and its pause of everything among it.
    expect(standingOf(work({ kind: 'paused', at: 'plan', by: 'engine', why: 'failed' }, [wait('pause')]))).toBe(
      'owner',
    );
    expect(standingOf(work({ kind: 'everything', at: 'plan', by: 'engine' }, [wait('held')]))).toBe('owner');
    expect(standingOf(work({ kind: 'paused', at: 'plan', by: 'owner', why: null }))).toBe('paused');
    expect(standingOf(work({ kind: 'round', cell: 'plan', run: 1, round: 1, since: 0 }))).toBe('running');
    const atLimit = { kind: 'call', call: 2, who: 'owner', what: { kind: 'atLimit', limit: { of: 'returns', sentBack: 2 } } } as const;
    expect(standingOf(work({ kind: 'waits', at: 'plan', on: [atLimit, { kind: 'slot' }] }, [wait('limit')]))).toBe('owner');
    expect(standingOf(work({ kind: 'waits', at: 'plan', on: [{ kind: 'slot' }] }))).toBe('due');
    expect(standingOf(work({ kind: 'everything', at: 'plan', by: 'owner' }))).toBe('due');
    // What the engine's list does not hold is not the owner's, whatever the work does.
    expect(standingOf(work({ kind: 'paused', at: 'plan', by: 'engine', why: 'failed' }))).toBe('paused');
    expect(standingOf(work({ kind: 'next', cell: 'plan' }))).toBe('due');
    const tool = { kind: 'call', call: 3, who: null, what: { kind: 'tool', name: 'ci', input: '' } } as const;
    expect(standingOf(work({ kind: 'waits', at: 'build', on: [tool] }))).toBe('others');
    // A work held for main to move waits for what the owner does not give.
    const main = { kind: 'outside', source: 'main', cell: 'M1', why: 'What fails also fails on main.' } as const;
    expect(standingOf(work({ kind: 'waits', at: 'M2', on: [main] }))).toBe('others');
    const stalled = { kind: 'stalled', at: 'M2', on: [{ test: 'checked = yes', words: 'checked = yes' }], by: [{ cell: 'M1', why: { kind: 'nothingNew' } }] } as const;
    // A stalled work is shown as a fault is.
    expect(standingOf(work({ ...stalled, on: [...stalled.on], by: [...stalled.by], why: 'x' }))).toBe('halted');
  });
});

describe('the span', () => {
  // A time zone whose clocks go back an hour on 2026-10-25, inside the four weeks the test counts; CI runs in UTC,
  // where no week would be longer than another.
  const zone = process.env.TZ;
  beforeAll(() => {
    process.env.TZ = 'Europe/Berlin';
  });
  afterAll(() => {
    if (zone === undefined) delete process.env.TZ;
    else process.env.TZ = zone;
  });

  it("is this week, the four weeks to its end, or every week from the earliest, each from its Monday's midnight", () => {
    // A Thursday afternoon, in the browser's own time zone.
    const thursday = new Date(2026, 10, 12, 15, 30).getTime();
    const monday = (month: number, day: number) => new Date(2026, month, day).getTime();
    expect(weekStarts('week', thursday)).toEqual([monday(10, 9)]);
    // Midnight each Monday, wherever the clocks changed between them: the week the clocks went back in is an hour
    // longer than the others.
    const four = weekStarts('4w', thursday);
    expect(four).toEqual([monday(9, 19), monday(9, 26), monday(10, 2), monday(10, 9)]);
    const WEEK = 7 * 24 * 60 * 60 * 1000;
    expect(four.slice(1).map((start, i) => start - four[i])).toEqual([WEEK + 60 * 60 * 1000, WEEK, WEEK]);
    expect(nextWeekStart(thursday)).toBe(monday(10, 16));
    expect(weekStarts('all', thursday, new Date(2026, 9, 28, 10).getTime())).toEqual([
      monday(9, 26),
      monday(10, 2),
      monday(10, 9),
    ]);
    // A Sunday belongs to the week that began on the Monday before it.
    expect(weekStarts('week', new Date(2026, 10, 15, 23, 0).getTime())).toEqual([monday(10, 9)]);
  });
});

describe('what the span adds up to', () => {
  it('gives the share read from the cache, the averages of the merges, and sums', () => {
    expect(cacheShare(spent(0, 1000, 3000))).toBe(0.75);
    expect(cacheShare(spent(0))).toBeNull();
    expect(mergeAverages([])).toBeNull();
    expect(
      mergeAverages([
        { work: 1, item: 1, at: 5000, from: 1000, spent: spent(2_000_000) },
        { work: 2, item: 2, at: 9000, from: 7000, spent: spent(4_000_000) },
        { work: 3, item: 3, at: 9000, from: null, spent: spent(0) },
      ]),
    ).toEqual({ took: 3000, usdMicros: 2_000_000 });
    expect(summed([{ spent: spent(10) }, { spent: null }, { spent: spent(5) }]).usdMicros).toBe(15);
  });
});

describe('the settings the page changes', () => {
  it('are the keys the engine lets the page change, each with its value as the box shows it', () => {
    const keys = changeables(fixtureSettings());
    expect(keys.map((k) => [k.key, k.value, k.running, k.leftOut])).toEqual([
      ['owner.language', 'Chinese', null, false],
      ['agents.slots', '3', '2', false],
      ['agents.levels.1.model', 'claude-haiku-5-5', null, false],
      ['agents.levels.2.model', 'claude-sonnet-5-5', null, false],
      ['agents.levels.3.model', 'claude-opus-5-5', null, false],
      ['items.highest_level', '3', null, false],
      ['sessions.context_limit', '500000', '500000', true],
    ]);
    expect(unquoted('"a \\"b\\""')).toBe('a "b"');
    expect(unquoted("'c'")).toBe('c');
    expect(unquoted('12')).toBe('12');
  });

  it("refuse a value of another kind than the key's, and leave the rest to the engine", () => {
    const keys = changeables(fixtureSettings());
    const of = (key: string) => keys.find((k) => k.key === key)!;
    const levels = ['1', '2', '3'];
    expect(problem(of('agents.slots'), '4', levels)).toBeNull();
    expect(problem(of('agents.slots'), '64', levels)).toBeNull();
    // No slots at all is the engine's to refuse, as the change is saved.
    expect(problem(of('agents.slots'), '0', levels)).toBeNull();
    expect(problem(of('agents.slots'), '2.5', levels)).toBe('agents.slots is a whole number');
    expect(problem(of('owner.language'), ' English', levels)).toBeNull();
    expect(problem(of('owner.language'), 'Eng\nlish', levels)).toBe('owner.language is a name on one line');
    expect(problem(of('items.highest_level'), '9', levels)).toBe('items.highest_level is one of the levels: 1, 2, 3');
  });
});
