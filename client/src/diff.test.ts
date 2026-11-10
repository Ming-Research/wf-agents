import { describe, expect, it } from 'vitest';

import { MOST_CHANGED, changedShare, diffWords, markedChange } from './diff';

const TASK = 'Write the plan from the request.\nSay what it leaves out.\n\nKeep it short.';

describe('a change of a text marked word by word', () => {
  it('marks nothing in two texts that are the same', () => {
    expect(diffWords(TASK, TASK)).toEqual([{ kind: 'same', text: TASK }]);
    expect(changedShare(diffWords(TASK, TASK))).toBe(0);
    expect(markedChange(TASK, TASK)).toEqual([{ kind: 'same', text: TASK }]);
  });

  it('marks one word changed as that word taken out and the new one put in', () => {
    const after = TASK.replace('short', 'brief');
    expect(diffWords(TASK, after)).toEqual([
      { kind: 'same', text: 'Write the plan from the request.\nSay what it leaves out.\n\nKeep it ' },
      { kind: 'del', text: 'short.' },
      { kind: 'ins', text: 'brief.' },
    ]);
    expect(markedChange(TASK, after)).not.toBeNull();
  });

  it('marks a word put in at the end, with the space before it', () => {
    expect(diffWords('Keep it short', 'Keep it short today')).toEqual([
      { kind: 'same', text: 'Keep it short' },
      { kind: 'ins', text: ' today' },
    ]);
  });

  it('marks a paragraph taken out, with its line breaks', () => {
    const before = 'First paragraph.\n\nSecond one, to go.\n\nThird paragraph.';
    const after = 'First paragraph.\n\nThird paragraph.';
    const parts = diffWords(before, after);
    expect(parts.filter((p) => p.kind !== 'same')).toEqual([{ kind: 'del', text: 'Second one, to go.\n\n' }]);
    expect(parts.filter((p) => p.kind !== 'ins').map((p) => p.text).join('')).toBe(before);
    expect(parts.filter((p) => p.kind !== 'del').map((p) => p.text).join('')).toBe(after);
  });

  it('marks a line break put in, and no word', () => {
    const before = 'Write the plan.\nSay what it leaves out.';
    const after = 'Write the plan.\n\nSay what it leaves out.';
    expect(diffWords(before, after)).toEqual([
      { kind: 'same', text: 'Write the plan.\n' },
      { kind: 'ins', text: '\n' },
      { kind: 'same', text: 'Say what it leaves out.' },
    ]);
    expect(changedShare(diffWords(before, after))).toBe(0);
  });

  it('takes a space between two changes into them', () => {
    expect(diffWords('one two three four', 'one five six four')).toEqual([
      { kind: 'same', text: 'one ' },
      { kind: 'del', text: 'two three' },
      { kind: 'ins', text: 'five six' },
      { kind: 'same', text: ' four' },
    ]);
  });

  it('marks all of a text put in where there was none, or taken out where there is none, and reads it whole', () => {
    expect(diffWords('', TASK)).toEqual([{ kind: 'ins', text: TASK }]);
    expect(diffWords(TASK, '')).toEqual([{ kind: 'del', text: TASK }]);
    expect(changedShare(diffWords('', TASK))).toBe(1);
    expect(markedChange('', TASK)).toBeNull();
    expect(markedChange(TASK, '')).toBeNull();
  });

  it('reads two texts whole where more than half of their words changed', () => {
    const before = 'one two three four five six';
    const half = 'one two three 4 5 6';
    const most = 'one two 3 4 5 6';
    expect(changedShare(diffWords(before, half))).toBe(MOST_CHANGED);
    expect(markedChange(before, half)).not.toBeNull();
    expect(changedShare(diffWords(before, most))).toBeGreaterThan(MOST_CHANGED);
    expect(markedChange(before, most)).toBeNull();
  });

  it('gives back both texts: what stayed and was taken out is the one before, what stayed and was put in the one after', () => {
    const before = 'Review the code.\nSay whether you are content.\n\nName each finding.';
    const after = 'Review the code and its tests.\nSay whether you are content with them.\nName each finding by its file.';
    const parts = diffWords(before, after);
    expect(parts.filter((p) => p.kind !== 'ins').map((p) => p.text).join('')).toBe(before);
    expect(parts.filter((p) => p.kind !== 'del').map((p) => p.text).join('')).toBe(after);
  });
});
