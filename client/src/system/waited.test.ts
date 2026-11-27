import { describe, expect, it } from 'vitest';

import type { Occupancy, Waited } from './api';
import {
  crowdWords,
  endedWords,
  slotsWords,
  takingTitle,
  takingWords,
  WAIT_REASONS,
  WAIT_WORDS,
  waitedBy,
  weekCells,
  works,
} from './waited';

describe("what the system's Waited part says", () => {
  it('names each cause once, in the order the engine counts them, with what it counts said on pointing at it', () => {
    expect(WAIT_REASONS).toEqual(['owner', 'items', 'slot', 'turn', 'paused']);
    const names = WAIT_REASONS.map((r) => WAIT_WORDS[r].name);
    expect(new Set(names).size).toBe(names.length);
    for (const reason of WAIT_REASONS) expect(WAIT_WORDS[reason].title.length).toBeGreaterThan(10);
  });

  it('puts each cause in its place and gives one the engine left out as none, not as nothing said', () => {
    const waited = {
      by: [{ reason: 'slot', stops: 2, totalMs: 5000, medianMs: 1000, longestMs: 4000 }],
    } as Waited;
    expect(waitedBy(waited).map((b) => [b.reason, b.stops])).toEqual([
      ['owner', 0],
      ['items', 0],
      ['slot', 2],
      ['turn', 0],
      ['paused', 0],
    ]);
  });

  it('says the median and the longest only of stops that ended', () => {
    expect(endedWords({ stops: 0, totalMs: 9000, medianMs: null, longestMs: null })).toBeUndefined();
    expect(endedWords({ stops: 3, totalMs: 9000, medianMs: 60_000, longestMs: 3_600_000 })).toBe(
      'median 1m 0s, longest 1h 0m',
    );
  });

  it('says how many works held a slot to a tenth of a work, and the slots there were where the engine kept them', () => {
    expect(works(2450)).toBe('2.5');
    expect(works(0)).toBe('0.0');
    expect(crowdWords(2400, 3, 16)).toBe('2.4 avg, 3 at most, of 16');
    expect(crowdWords(2400, 3, null)).toBe('2.4 avg, 3 at most, of not kept');
    expect(crowdWords(2900, 4)).toBe('2.9 avg, 4 at most');
  });

  it('says of the slots that the engine kept them for only part of a stretch, never as the slots of the whole of it', () => {
    expect(slotsWords(16, true)).toBe('16');
    expect(slotsWords(16, false)).toBe('16 (kept in part)');
    expect(slotsWords(null, false)).toBe('not kept');
    expect(crowdWords(2400, 3, 16, false)).toBe('2.4 avg, 3 at most, of 16 (kept in part)');
    const occupancy = { takingAvg: 2400, takingPeak: 3, runningAvg: 2900, runningPeak: 4, slots: 3, recorded: true } as Occupancy;
    expect(takingWords(occupancy)).toBe('2.4 avg, 3 at most, of 3');
    expect(takingWords({ ...occupancy, recorded: false })).toBe('2.4 avg, 3 at most, of 3 (kept in part)');
    expect(takingTitle(occupancy)).not.toContain('only from the time it began to');
    expect(takingTitle({ ...occupancy, recorded: false })).toContain('only from the time it began to');
  });

  it('gives a week of an engine that does not say as no figure, and a week it says nothing waited in as none', () => {
    const week = {
      start: 0,
      spent: {},
      merged: 0,
      cancelled: 0,
      added: 0,
      sentBack: 0,
      failed: 0,
    } as never;
    expect(weekCells(week)).toEqual(['', '', '', '', '']);
    expect(weekCells({ ...(week as object), waited: { totalMs: [0, 60_000, 0, 0, 0], ciMs: 0 } } as never)).toEqual([
      'none',
      '1m 0s',
      'none',
      'none',
      'none',
    ]);
  });
});
