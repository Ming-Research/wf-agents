import { describe, expect, it } from 'vitest';

import type { TestKind } from './api';
import { test } from './testing';
import { changeWords, conditionWords, faultWords, readsWords, writersBesideCells } from './words';

describe('the words of the page', () => {
  it('say a condition by its block and value, or by its test’s name, as the cell’s file writes it', () => {
    expect(conditionWords({ on: 'block', block: 'needs_design', value: 'yes' })).toBe('needs_design is yes');
    expect(
      conditionWords({ on: 'test', test: test('has_comment', 'The item has a comment', 'any', ['commented']) }),
    ).toBe('has_comment');
    expect(conditionWords(null)).toBe('');
  });

  it('say a test by its keys as its file writes them, for one block and for several', () => {
    const reads = (kind: TestKind, blocks: string[]) => readsWords(test('t', '', kind, blocks));
    expect(reads('all', ['go'])).toBe('kind all, reads [go], is yes');
    expect(reads('none', ['needs_design', 'edits_tree'])).toBe('kind none, reads [needs_design, edits_tree], is yes');
    expect(reads('noneStanding', ['verdict', 'deep_verdict'])).toBe(
      'kind none_standing, reads [verdict, deep_verdict], is yes',
    );
  });

  it('say the source outside that writes a block by its key', () => {
    const block = {
      name: 'main',
      values: null,
      what: null,
      writtenBy: { cells: [], people: true, note: false, outside: 'main' },
      readBy: { cells: [], conditions: [], tests: [], shown: false },
    };
    expect(writersBesideCells(block)).toEqual(['people', 'outside main']);
  });

  it('say a change by the key it changed, as the files write it, and its value before and after', () => {
    expect(changeWords({ kind: 'cellMoved', cell: 'a', before: 3, after: 0 })).toEqual({
      what: 'order',
      before: '4',
      after: '1',
    });
    expect(
      changeWords({ kind: 'condition', cell: 'a', before: null, after: { on: 'block', block: 'x', value: 'no' } }),
    ).toEqual({ what: 'when', before: '', after: 'x is no' });
    expect(changeWords({ kind: 'runsOnRemoved', cell: 'a', block: 'x' })).toEqual({
      what: 'triggered_by',
      before: 'x',
      after: '',
    });
    expect(changeWords({ kind: 'givenAdded', cell: 'a', block: 'x' })).toEqual({
      what: 'context',
      before: '',
      after: 'x',
    });
    expect(changeWords({ kind: 'toolAdded', cell: 'a', tool: 'query' })).toEqual({
      what: 'tools',
      before: '',
      after: 'query',
    });
    expect(changeWords({ kind: 'system', cell: 'a', before: [], after: ['general', 'engineering'] })).toEqual({
      what: 'rules',
      before: '[]',
      after: '[general, engineering]',
    });
    expect(changeWords({ kind: 'givenOrder', cell: 'a', before: ['x', 'y'], after: ['y', 'x'] })).toEqual({
      what: 'context',
      before: '[x, y]',
      after: '[y, x]',
    });
    expect(changeWords({ kind: 'rulesChanged', name: 'general', before: 'One.', after: 'Two.' })).toEqual({
      what: 'rules/general.md',
      before: 'One.',
      after: 'Two.',
    });
    expect(changeWords({ kind: 'givenKind', cell: 'a', block: 'x', before: 'lastAndLatest', after: 'all' })).toEqual({
      what: 'kinds.x',
      before: 'last_and_latest',
      after: 'all',
    });
    expect(changeWords({ kind: 'runsWhy', cell: 'a', block: 'x', before: null, after: 'It is new.' })).toEqual({
      what: 'run.x.why',
      before: '',
      after: 'It is new.',
    });
    expect(changeWords({ kind: 'readsOnly', cell: 'a', before: false, after: true })).toEqual({
      what: 'reads_only',
      before: 'false',
      after: 'true',
    });
    expect(changeWords({ kind: 'mustWriteAdded', cell: 'a', block: 'x' })).toEqual({
      what: 'must_write',
      before: '',
      after: 'x',
    });
    expect(changeWords({ kind: 'mustWriteRemoved', cell: 'a', block: 'x' })).toEqual({
      what: 'must_write',
      before: 'x',
      after: '',
    });
    expect(changeWords({ kind: 'effort', cell: 'a', before: 'high', after: 'xhigh' })).toEqual({
      what: 'effort',
      before: 'high',
      after: 'xhigh',
    });
    expect(changeWords({ kind: 'levelsAbove', cell: 'a', before: 0, after: 1 })).toEqual({
      what: 'levels_above',
      before: '0',
      after: '1',
    });
    expect(changeWords({ kind: 'describesAdded', cell: 'a', block: 'x' })).toEqual({
      what: 'describes',
      before: '',
      after: 'x',
    });
    expect(changeWords({ kind: 'values', block: 'x', before: null, after: ['yes', 'no'] })).toEqual({
      what: 'values',
      before: '',
      after: '[yes, no]',
    });
  });

  it('say a test put in, changed or required by its keys, and a block’s source by its name', () => {
    const before = test('may_merge', 'The CI is green', 'all', ['ci'], 'green');
    const after = test('may_merge', 'The work may merge', 'none', ['ci'], 'red');
    expect(changeWords({ kind: 'testChanged', test: 'may_merge', before, after })).toEqual({
      what: 'may_merge',
      before: 'kind all, reads [ci], is green',
      after: 'kind none, reads [ci], is red',
    });
    expect(changeWords({ kind: 'testAdded', test: after })).toEqual({
      what: 'may_merge',
      before: '',
      after: 'kind none, reads [ci], is red',
    });
    expect(changeWords({ kind: 'preconditionRemoved', cell: 'merge', test: before })).toEqual({
      what: 'requires',
      before: 'may_merge',
      after: '',
    });
    expect(changeWords({ kind: 'task', cell: 'plan', before: 'Plan it.', after: 'Plan it well.' })).toEqual({
      what: 'task',
      before: 'Plan it.',
      after: 'Plan it well.',
    });
    expect(changeWords({ kind: 'act', cell: 'check', before: 'check', after: null })).toEqual({
      what: 'act',
      before: 'check',
      after: '',
    });
    expect(changeWords({ kind: 'outside', block: 'main', before: null, after: 'main' })).toEqual({
      what: 'outside',
      before: '',
      after: 'main',
    });
  });

  it('say that a block is a note, a change of it, and a note people do not write', () => {
    const block = {
      name: 'opinions',
      values: null,
      what: null,
      writtenBy: { cells: [], people: true, note: true, outside: null },
      readBy: { cells: ['plan'], conditions: [], tests: [], shown: false },
    };
    expect(writersBesideCells(block)).toEqual(['people', 'note']);
    expect(changeWords({ kind: 'note', block: 'opinions', before: false, after: true })).toEqual({
      what: 'note',
      before: 'false',
      after: 'true',
    });
    expect(
      faultWords({ file: 'flow.toml', field: 'blocks.plan.note', what: { kind: 'noteNotByPeople', block: 'plan' } }),
    ).toBe('plan is a note, but people do not write it.');
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
      faultWords({ file: 'judge.toml', field: 'must_write[1]', what: { kind: 'notAnOutput', block: 'plan' } }),
    ).toBe('The cell must write plan every time, but plan is not among the blocks it may write.');
    expect(
      faultWords({ file: 'check.toml', field: 'describes[0]', what: { kind: 'describedNotRunOn', block: 'ci' } }),
    ).toBe('The cell says what the work is by ci, but ci does not run it.');
    expect(faultWords({ file: 'plan.toml', field: 'give.request', what: { kind: 'notGiven', block: 'request' } })).toBe(
      'The cell says how it is given request, but it is not given request.',
    );
    expect(
      faultWords({
        file: 'flow.toml',
        field: 'lines.engineer.system[1]',
        what: { kind: 'noSuchRules', name: 'general' },
      }),
    ).toBe('The line names the rules general, but the flow has no rules/general.md.');
    expect(
      faultWords({ file: 'check.toml', field: 'input.main.kind', what: { kind: 'engineIsGiven', block: 'main' } }),
    ).toBe("An engine's cell says how it is given main, but it has no session.");
    expect(faultWords({ file: 'plan.toml', field: 'inputs', what: { kind: 'twoForms' } })).toBe(
      'Name them one way: inputs, runs_on and given, or triggered_by and context.',
    );
    expect(
      faultWords({
        file: 'cells/plan.toml',
        field: 'triggered_by[3]',
        what: { kind: 'noSuchTrigger', trigger: 'nothing_happened' },
      }),
    ).toBe('The cell names the trigger nothing_happened, which nothing in ../triggers/ defines.');
    expect(
      faultWords({
        file: 'cells/judge.toml',
        field: 'triggered_by[1]',
        what: { kind: 'readNotInContext', trigger: 'on_request', block: 'request' },
      }),
    ).toBe("The trigger on_request has the agent read request, but request is not in the cell's context.");
    expect(
      faultWords({
        file: 'cells/plan.toml',
        field: 'triggered_by[2]',
        what: { kind: 'noSuchDatum', trigger: 'on_ghost', datum: 'ghost' },
      }),
    ).toBe('The cell names the trigger on_ghost, but the flow has no datum ghost.');
    expect(
      faultWords({
        file: 'blocks/plan.toml',
        field: '',
        what: { kind: 'definedTwice', name: 'plan', also: 'blocks/other/plan.toml' },
      }),
    ).toBe('plan is defined here and again in blocks/other/plan.toml.');
    expect(
      faultWords({ file: 'judge.toml', field: 'input.opinions', what: { kind: 'takenNotRead', block: 'opinions' } }),
    ).toBe('The cell says how it takes opinions, but opinions is not among the blocks it reads.');
    expect(faultWords({ file: 'plan.toml', field: 'runs_on', what: { kind: 'runByNothing' } })).toBe(
      "Nothing runs this agent's cell, so it would never be entered.",
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
        file: 'plan.toml',
        field: 'requires[0]',
        what: { kind: 'testWrittenAfter', test: 'may_merge', block: 'ci', cell: 'check' },
      }),
    ).toBe('The test may_merge reads ci, which check, a cell after this one, writes.');
    expect(
      faultWords({
        file: 'flow.toml',
        field: 'tests.approved.reads[0]',
        what: { kind: 'standingWrittenByTwo', test: 'approved', block: 'verdict', cell: 'deep' },
      }),
    ).toBe(
      'The test approved, of what stands, reads verdict, which deep writes as well as a cell before it: a skipped ' +
        "cell's write would hide the other's word.",
    );
    expect(
      faultWords({
        file: 'check.toml',
        field: 'when.test',
        what: { kind: 'testOnOwnOutput', test: 'may_merge', block: 'ci' },
      }),
    ).toBe(
      'The test may_merge reads ci, which the cell itself writes: the cell would decide by what it writes itself ' +
        'whether the work waits before it, or, beside another writer, whether it runs.',
    );
    expect(
      faultWords({
        file: 'design.toml',
        field: 'when.block',
        what: { kind: 'conditionNotWrittenBefore', block: 'go' },
      }),
    ).toBe('The condition reads go, which neither a cell before this one, people nor the outside write.');
    expect(faultWords({ file: 'plan.toml', field: 'task', what: { kind: 'noTask' } })).toBe(
      "An agent's cell has no task, or one of no words: its agent would be told nothing to do.",
    );
    expect(faultWords({ file: 'check.toml', field: 'task', what: { kind: 'engineHasTask' } })).toBe(
      "An engine's cell has a task, which no agent reads.",
    );
    expect(faultWords({ file: 'check.toml', field: 'act', what: { kind: 'noAct' } })).toBe(
      "An engine's cell names no act to carry it out.",
    );
    expect(faultWords({ file: 'plan.toml', field: 'act', what: { kind: 'agentHasAct', act: 'merge' } })).toBe(
      'An agent carries the cell out, but the cell names the act "merge".',
    );
    expect(faultWords({ file: 'check.toml', field: 'effort', what: { kind: 'engineHasEffort' } })).toBe(
      "An engine's cell names a reasoning effort, which no agent runs at.",
    );
    expect(faultWords({ file: 'plan.toml', field: 'effort', what: { kind: 'noEffort' } })).toBe(
      "An agent's cell names no reasoning effort for its rounds to run at.",
    );
    expect(faultWords({ file: 'check.toml', field: 'act', what: { kind: 'unknownAct', act: 'deploy' } })).toBe(
      'The engine has no act named "deploy".',
    );
    // A name padded or empty shows as it is written.
    expect(faultWords({ file: 'check.toml', field: 'act', what: { kind: 'unknownAct', act: 'merge ' } })).toBe(
      'The engine has no act named "merge ".',
    );
  });
});
