import { describe, expect, it } from 'vitest';

import { inOrder } from './source';

/// Lets every answer given so far be taken.
const flush = () => new Promise<void>((done) => setTimeout(done, 0));

describe('reads in order', () => {
  /// A read the test answers when it likes.
  function deferred<T>() {
    let answer!: (v: T) => void;
    let refuse!: (e: Error) => void;
    const promise = new Promise<T>((yes, no) => {
      answer = yes;
      refuse = no;
    });
    return { promise, answer, refuse };
  }

  it('drops an answer older than the one shown, and a failure counts as an answer', async () => {
    const take = inOrder();
    const shown: string[] = [];
    const first = deferred<string>();
    const second = deferred<string>();
    const third = deferred<string>();
    take(first.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    take(second.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    take(third.promise, (v) => shown.push(v), (why) => shown.push(`failed: ${why}`));
    second.refuse(new Error('gone'));
    await flush();
    first.answer('first');
    await flush();
    third.answer('third');
    await flush();
    expect(shown).toEqual(['failed: gone', 'third']);
  });
});
