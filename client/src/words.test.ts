import { describe, expect, it } from 'vitest';

import type { TestKind } from './api';
import { test } from './testing';
import { conditionWords, faultWords, readsWords, writersBesideCells } from './words';

describe('the words of the page', () => {
  it('say a when or a requires as the cell’s file writes it, every key: a block with its value, a test by its name', () => {
    expect(conditionWords([{ on: 'block', block: 'needs_design', value: 'yes' }])).toBe('needs_design = yes');
    const comment = test('has_comment', 'The item has a comment', 'any', ['commented']);
    expect(conditionWords([{ on: 'block', block: 'deep_needed', value: 'no' }, { on: 'test', test: comment }])).toBe(
      'deep_needed = no, test = has_comment',
    );
    expect(conditionWords([])).toBe('');
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
      default: null,
      what: null,
      writtenBy: { cells: [], people: true, note: false, outside: 'main' },
      readBy: { cells: [], conditions: [], tests: [], shown: false },
    };
    expect(writersBesideCells(block)).toEqual(['people', 'outside main']);
  });

  it('say that a block is a note, and a note people do not write', () => {
    const block = {
      name: 'opinions',
      values: null,
      default: null,
      what: null,
      writtenBy: { cells: [], people: true, note: true, outside: null },
      readBy: { cells: ['plan'], conditions: [], tests: [], shown: false },
    };
    expect(writersBesideCells(block)).toEqual(['people', 'note']);
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
    expect(faultWords({ file: 'cells/a.toml', field: 'tools[2]', what: { kind: 'twice', name: 'query' } })).toBe(
      'query is named a second time in the same list.',
    );
    expect(faultWords({ file: 'cells/a.toml', field: 'tools', what: { kind: 'twice', name: 'tools' } })).toBe(
      'The cell names tools of its own, and the flow already gives its agent cells theirs.',
    );
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
    expect(
      faultWords({ file: 'blocks/plan.toml', field: 'default', what: { kind: 'defaultNotAChoice', block: 'plan' } }),
    ).toBe('plan says what it counts as before it is written, but it is not a choice.');
    expect(
      faultWords({
        file: 'blocks/plan/deep_needed.toml',
        field: 'default',
        what: { kind: 'defaultNotAValue', block: 'deep_needed', default: 'maybe' },
      }),
    ).toBe('deep_needed counts as maybe before it is written, which is not one of its values.');
    expect(faultWords({ file: 'blocks/test.toml', field: '', what: { kind: 'blockNamedTest' } })).toBe(
      "A block is named test, which a cell's when takes for a named test.",
    );
    expect(faultWords({ file: 'cells/D1.toml', field: 'when', what: { kind: 'emptyCondition' } })).toBe(
      "A cell's when or requires has no keys: it says nothing that must hold.",
    );
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
