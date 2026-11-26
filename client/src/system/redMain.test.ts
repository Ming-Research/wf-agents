import { describe, expect, it } from 'vitest';

import type { RedMain } from './api';
import { redMainWords } from './redMain';

const SINCE = Date.UTC(2026, 10, 25, 14, 3);

function red(over: Partial<RedMain> = {}): RedMain {
  return {
    project: 'Firn-wf',
    since: SINCE,
    tests: [{ unit: 'firn', name: 'firn_runs_scripts', head: 'f4e3d6c0123456789' }],
    waiting: [184, 190],
    ...over,
  };
}

describe('the words of a project whose main is red', () => {
  it('name the test, the head it fails on and how many works wait', () => {
    const said = redMainWords(red());
    expect(said).toContain('firn_runs_scripts fails on its head f4e3d6c01234,');
    expect(said).toContain('2 works wait for it to pass');
  });

  it('say that no work waits, and one work in the singular', () => {
    expect(redMainWords(red({ waiting: [] }))).toContain('no work waits for it');
    expect(redMainWords(red({ waiting: [7] }))).toContain('1 work waits for it to pass');
  });

  it('count the tests past the first few', () => {
    const tests = Array.from({ length: 8 }, (_, n) => ({ unit: 'u', name: `t${n}`, head: 'abc' }));
    const said = redMainWords(red({ tests }));
    expect(said).toContain('t0, t1, t2, t3, t4 and 3 more fail on its head abc');
    expect(said).not.toContain('t5');
  });
});
