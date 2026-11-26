import { describe, expect, it } from 'vitest';

import { BAR_KINDS, TRACK_KINDS, TRACK_WORDS, barSpans } from './track';

describe("a work's time track", () => {
  it('draws a stretch nothing ran in as what the work waited for, each run of a character one block, its bare ends left out', () => {
    const bar = `..mm${'o'.repeat(4)}m${'p'.repeat(2)}t${'w'.repeat(2)}.m..`;
    const spans = barSpans(bar);
    expect(spans.map((s) => s.kind)).toEqual(['model', 'owner', 'model', 'paused', 'tool', 'items', 'gap', 'model']);
    const whole = [...bar].length;
    expect(spans[1]).toEqual({ key: '4', kind: 'owner', left: (4 / whole) * 100, width: (4 / whole) * 100 });
    // The ends before the first block and after the last are bare.
    expect(spans[0].left).toBe((2 / whole) * 100);
    const last = spans[spans.length - 1];
    expect(last.left + last.width).toBeCloseTo(((whole - 2) / whole) * 100);
  });

  it('knows every character the engine writes, and says what each kind is on pointing at it', () => {
    expect(Object.keys(BAR_KINDS).sort()).toEqual(['.', '?', 'm', 'o', 'p', 'q', 's', 't', 'w']);
    // Every kind but a wait for a slot, which is drawn as nothing having run, has its colour in the legend.
    expect([...TRACK_KINDS].sort()).toEqual([...new Set(Object.values(BAR_KINDS))].filter((k) => k !== 'slot').sort());
    for (const kind of Object.values(BAR_KINDS)) expect(TRACK_WORDS[kind]).toBeTruthy();
    expect(barSpans('....')).toEqual([]);
  });

  it('draws a wait for a slot as a block of its own that says so on pointing at it, and not as the stretch nothing says', () => {
    const spans = barSpans('mmsssm');
    expect(spans.map((s) => s.kind)).toEqual(['model', 'slot', 'model']);
    expect(spans[1]).toMatchObject({ left: (2 / 6) * 100, width: (3 / 6) * 100 });
    expect(TRACK_WORDS.slot).toContain('Waiting for a slot');
    expect(TRACK_WORDS.slot).not.toBe(TRACK_WORDS.gap);
    // It is no colour of the legend: it is drawn in the colour of nothing having run.
    expect(TRACK_KINDS).not.toContain('slot');
  });

  it('draws a stretch whose wait is not known, of a work that kept no time, as a block of its own that says so', () => {
    const spans = barSpans('m??t');
    expect(spans.map((s) => s.kind)).toEqual(['model', 'unknown', 'tool']);
    expect(TRACK_WORDS.unknown).toContain('not known');
    expect(TRACK_WORDS.unknown).not.toBe(TRACK_WORDS.gap);
  });

  it("draws a wait for the project's turn to check and merge as a block of its own, saying so on pointing at it", () => {
    const spans = barSpans('mmqqqm');
    expect(spans.map((s) => s.kind)).toEqual(['model', 'turn', 'model']);
    expect(spans[1]).toMatchObject({ left: (2 / 6) * 100, width: (3 / 6) * 100 });
    expect(TRACK_WORDS.turn).toBe('Waiting for its turn to check and merge');
    for (const kind of TRACK_KINDS) if (kind !== 'turn') expect(TRACK_WORDS[kind]).not.toBe(TRACK_WORDS.turn);
  });
});
