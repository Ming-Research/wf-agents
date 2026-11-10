import { describe, expect, it } from 'vitest';

import { changeWords, conditionWords, faultWords } from './words';

describe('the words of the page', () => {
  it('say a condition by its block and value, or by its test’s words', () => {
    expect(conditionWords({ on: 'block', block: 'needs_design', value: 'yes' })).toBe('needs_design is yes');
    expect(conditionWords({ on: 'test', test: 'has_comment', words: 'The item has a comment' })).toBe(
      'The item has a comment',
    );
    expect(conditionWords(null)).toBe('always');
  });

  it('say a change as what changed, what it was and what it is', () => {
    expect(changeWords({ kind: 'cellMoved', cell: 'a', before: 3, after: 0 })).toEqual({
      what: 'Moved',
      before: 'place 4',
      after: 'place 1',
    });
    expect(
      changeWords({ kind: 'condition', cell: 'a', before: null, after: { on: 'block', block: 'x', value: 'no' } }),
    ).toEqual({ what: 'Runs when', before: 'always', after: 'x is no' });
    expect(changeWords({ kind: 'inputRemoved', cell: 'a', block: 'x' })).toEqual({ what: 'Reads', before: 'x', after: '' });
    expect(changeWords({ kind: 'values', block: 'x', before: null, after: ['yes', 'no'] })).toEqual({
      what: 'Values',
      before: 'not a choice',
      after: 'yes, no',
    });
  });

  it('say what is wrong with a definition from the data of the fault', () => {
    expect(faultWords({ file: 'plan.toml', field: 'inputs[2]', what: { kind: 'noSuchBlock', block: 'ghost' } })).toBe(
      'The block ghost is not declared in flow.toml.',
    );
    expect(
      faultWords({ file: 'flow.toml', field: 'order', what: { kind: 'flowRefused', reason: 'twice', cell: 'a' } }),
    ).toBe('Two cells have the same name. (a)');
  });
});
