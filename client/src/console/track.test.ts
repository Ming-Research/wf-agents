import { describe, expect, it } from 'vitest';

import { BAR_KINDS, TRACK_WORDS, barSpans } from './track';

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
    expect(Object.keys(BAR_KINDS).sort()).toEqual(['.', 'm', 'o', 'p', 't', 'w']);
    for (const kind of Object.values(BAR_KINDS)) expect(TRACK_WORDS[kind]).toBeTruthy();
    expect(barSpans('....')).toEqual([]);
  });
});
