import { describe, expect, it } from 'vitest';

import { test } from './testing';
import { changeWords, conditionWords, faultWords, readsWords, writersBesideCells } from './words';

describe('the words of the page', () => {
  it('say a condition by its block and value, or by its test’s words and what the test reads', () => {
    expect(conditionWords({ on: 'block', block: 'needs_design', value: 'yes' })).toBe('needs_design is yes');
    expect(conditionWords({ on: 'test', test: test('has_comment', 'The item has a comment', 'any', ['commented']) })).toBe(
      'The item has a comment (commented is yes)',
    );
    expect(conditionWords(null)).toBe('always');
  });

  it('say what a test reads by its kind, for one block and for several', () => {
    const reads = (kind: 'all' | 'any' | 'none', blocks: string[]) => readsWords(test('t', '', kind, blocks));
    expect(reads('all', ['go'])).toBe('go is yes');
    expect(reads('any', ['go'])).toBe('go is yes');
    expect(reads('none', ['go'])).toBe('go is not yes');
    expect(reads('all', ['needs_design', 'changes_tree'])).toBe('all of needs_design, changes_tree are yes');
    expect(reads('any', ['needs_design', 'changes_tree'])).toBe('any of needs_design, changes_tree is yes');
    expect(reads('none', ['needs_design', 'changes_tree'])).toBe('none of needs_design, changes_tree is yes');
  });

  it('say the source outside that writes a block', () => {
    const block = {
      name: 'main',
      values: null,
      writtenBy: { cells: [], people: true, outside: 'main' },
      readBy: { cells: [], conditions: [], tests: [], shown: false },
    };
    expect(writersBesideCells(block)).toEqual(['people', 'the outside (main)']);
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

  it('say a test put in, changed or required with what it reads, and a block’s source by its name', () => {
    const before = test('may_merge', 'The CI is green', 'all', ['ci'], 'green');
    const after = test('may_merge', 'The work may merge', 'none', ['ci'], 'red');
    expect(changeWords({ kind: 'testChanged', test: 'may_merge', before, after })).toEqual({
      what: 'Holds when',
      before: 'ci is green',
      after: 'ci is not red',
    });
    expect(changeWords({ kind: 'testAdded', test: after })).toEqual({
      what: 'Declared',
      before: '',
      after: 'The work may merge (ci is not red)',
    });
    expect(changeWords({ kind: 'preconditionRemoved', cell: 'merge', test: before })).toEqual({
      what: 'Waits until',
      before: 'The CI is green (ci is green)',
      after: '',
    });
    expect(changeWords({ kind: 'task', cell: 'plan', before: 'Plan it.', after: 'Plan it well.' })).toEqual({
      what: 'Task',
      before: 'Plan it.',
      after: 'Plan it well.',
    });
    expect(changeWords({ kind: 'act', cell: 'check', before: 'check', after: null })).toEqual({
      what: 'Act',
      before: 'check',
      after: 'none',
    });
    expect(changeWords({ kind: 'outside', block: 'main', before: null, after: 'main' })).toEqual({
      what: 'Written by the outside',
      before: 'no',
      after: 'main',
    });
  });

  it('say what is wrong with a definition from the data of the fault', () => {
    expect(faultWords({ file: 'plan.toml', field: 'inputs[2]', what: { kind: 'noSuchBlock', block: 'ghost' } })).toBe(
      'The block ghost is not declared in flow.toml.',
    );
    expect(
      faultWords({ file: 'flow.toml', field: 'order', what: { kind: 'flowRefused', reason: 'twice', cell: 'a' } }),
    ).toBe('Two cells have the same name. (a)');
    expect(
      faultWords({
        file: 'flow.toml',
        field: 'blocks.main.outside',
        what: { kind: 'unknownSource', block: 'main', source: 'mirror' },
      }),
    ).toBe('main is written by the source mirror, which the engine does not have.');
    expect(faultWords({ file: 'flow.toml', field: 'tests.t.reads', what: { kind: 'readsEmpty', test: 't' } })).toBe(
      'The test t reads no block.',
    );
    expect(
      faultWords({
        file: 'plan.toml',
        field: 'requires[0]',
        what: { kind: 'testNotWrittenBefore', test: 'may_merge', block: 'ci' },
      }),
    ).toBe('The test may_merge reads ci, which neither a cell before this one, people nor the outside write.');
    expect(
      faultWords({
        file: 'check.toml',
        field: 'when.test',
        what: { kind: 'testOnOwnOutput', test: 'may_merge', block: 'ci' },
      }),
    ).toBe(
      'The test may_merge reads ci, which the cell itself writes: the cell would decide by what it writes itself ' +
        'whether it runs, or whether the work waits before it.',
    );
    expect(
      faultWords({ file: 'design.toml', field: 'when.block', what: { kind: 'conditionNotWrittenBefore', block: 'go' } }),
    ).toBe('The condition reads go, which neither a cell before this one, people nor the outside write.');
    expect(faultWords({ file: 'plan.toml', field: 'task', what: { kind: 'noTask' } })).toBe(
      "An agent's cell has no task: its agent would be told nothing to do.",
    );
    expect(faultWords({ file: 'check.toml', field: 'task', what: { kind: 'engineHasTask' } })).toBe(
      "An engine's cell has a task, which no agent reads.",
    );
    expect(faultWords({ file: 'check.toml', field: 'act', what: { kind: 'noAct' } })).toBe(
      "An engine's cell names no act to carry it out.",
    );
    expect(faultWords({ file: 'plan.toml', field: 'act', what: { kind: 'agentHasAct', act: 'merge' } })).toBe(
      'An agent carries the cell out, but it names the act merge.',
    );
    expect(faultWords({ file: 'check.toml', field: 'act', what: { kind: 'unknownAct', act: 'deploy' } })).toBe(
      'The engine has no act named deploy.',
    );
  });
});
