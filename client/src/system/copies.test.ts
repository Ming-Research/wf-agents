import { describe, expect, it } from 'vitest';

import type { Copies, CopiesState } from '../works-api';
import { fixtureCopies } from '../works/fixtures';
import { copiesTone, copiesWords } from './copies';

const HOUR = 60 * 60 * 1000;

/// The copies as the engine gives them in this state, at the fixtures' time.
function copiesIn(state: CopiesState, over: Partial<Copies> = {}): Copies {
  return { ...fixtureCopies(), state, ...over };
}

describe('the words of the copies of the database', () => {
  it('are plain where the newest copy is recent and kept off the machine', () => {
    const copies = copiesIn({ kind: 'ok' }, { lastTaken: fixtureCopies().now - 3 * HOUR });
    expect(copiesWords(copies)).toBe('The newest copy of the database was taken 3h 0m ago, and is kept off this machine');
    expect(copiesTone(copies)).toBe('quiet');
  });

  const warned: [string, CopiesState, 'warn' | 'failed', string][] = [
    ['kept on this machine only', { kind: 'localOnly' }, 'warn', 'stay on this machine, on the disk they guard against losing'],
    ['not yet kept off the machine', { kind: 'pending', n: 2 }, 'warn', '2 copies of the database not yet kept off this machine'],
    ['too old', { kind: 'stale', since: fixtureCopies().now - 30 * HOUR }, 'warn', 'was taken 1d 6h ago'],
    ['none taken', { kind: 'stale', since: null }, 'warn', 'No copy of the database has been taken'],
    ['dated in the future', { kind: 'clock' }, 'warn', "the engine's clock is wrong"],
    [
      'failed',
      { kind: 'failed', at: fixtureCopies().now - HOUR, step: 'upload', why: 'the place cannot be reached' },
      'failed',
      "Keeping the database's copies failed at upload: the place cannot be reached",
    ],
  ];
  for (const [name, state, tone, words] of warned) {
    it(`warn, with their reason, where the copies are ${name}`, () => {
      expect(copiesWords(copiesIn(state))).toContain(words);
      expect(copiesTone(copiesIn(state))).toBe(tone);
    });
  }

  it('say one copy in the singular', () => {
    expect(copiesWords(copiesIn({ kind: 'pending', n: 1 }))).toBe('1 copy of the database not yet kept off this machine');
  });
});
