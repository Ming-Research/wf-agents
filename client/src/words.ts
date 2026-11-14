// How the page writes what the interface gives as data: conditions, named tests and limits by their keys and values as
// the flow's files write them (principle definitions-shown-as-written), and faults in a sentence each.

import type { Block, Cell, Condition, Fault, Flow, InputKind, Test } from './api';

/// A name or a value as the files write it: a key of the interface in the files' own spelling.
export function snake(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

/// A list as the files write it.
export function listed(names: string[]): string {
  return `[${names.join(', ')}]`;
}

/// A named test by its attributes, as its file writes them.
export function readsWords(test: Test): string {
  return `kind ${snake(test.kind)}, reads ${listed(test.reads)}, is ${test.is}`;
}

/// A named test, wherever a cell's condition or precondition names it: by its name.
export function testWords(test: Test): string {
  return test.name;
}

/// A condition as the cell's file writes it: the block and the value it waits for, or the named test; nothing where
/// the cell has none.
export function conditionWords(condition: Condition | null): string {
  if (!condition) return '';
  return condition.on === 'block' ? `${condition.block} is ${condition.value}` : testWords(condition.test);
}

/// Who writes a block besides cells, by the block's own keys: `people`, `note`, `outside` with its source.
export function writersBesideCells(block: Block): string[] {
  const { people, note, outside } = block.writtenBy;
  return [
    ...(people ? ['people'] : []),
    ...(note ? ['note'] : []),
    ...(outside !== null ? [`outside ${outside}`] : []),
  ];
}

/// What is wrong, in a sentence.
export function faultWords(fault: Fault): string {
  const w = fault.what;
  switch (w.kind) {
    case 'unreadable':
      return `The file cannot be read: ${w.message}`;
    case 'orderEmpty':
      return 'The order names no cell.';
    case 'noCellFile':
      return `The order names ${w.cell}, which has no file.`;
    case 'notInOrder':
      return `The cell ${w.cell} has a file but is not named in the order.`;
    case 'twice':
      // At the field `tools` itself, a cell names tools of its own where the flow already gives them to every agent cell.
      return fault.field === 'tools'
        ? `The cell names ${w.name} of its own, and the flow already gives its agent cells theirs.`
        : `${w.name} is named a second time in the same list.`;
    case 'limitOutOfRange':
      return `The limit ${w.value} is not a whole number from 1 to the largest count the engine keeps.`;
    case 'noLine':
      return "An agent's cell names no line to run in.";
    case 'engineHasLine':
      return `An engine's cell runs in no session, but names the line ${w.line}.`;
    case 'noSuchLine':
      return `The line ${w.line} is not declared in flow.toml.`;
    case 'noTask':
      return "An agent's cell has no task, or one of no words: its agent would be told nothing to do.";
    case 'engineHasTask':
      return "An engine's cell has a task, which no agent reads.";
    case 'noAct':
      return "An engine's cell names no act to carry it out.";
    case 'agentHasAct':
      return `An agent carries the cell out, but the cell names the act "${w.act}".`;
    case 'engineHasEffort':
      return "An engine's cell names a reasoning effort, which no agent runs at.";
    case 'noEffort':
      return "An agent's cell names no reasoning effort for its rounds to run at.";
    case 'engineHasTools':
      return "An engine's cell names tools, which no agent asks for.";
    case 'unknownAct':
      return `The engine has no act named "${w.act}".`;
    case 'unknownTool':
      return `The engine has no tool named "${w.tool}".`;
    case 'noSuchRules':
      return `The line names the rules ${w.name}, but the flow has no rules/${w.name}.md.`;
    case 'enginePart':
      return `The engine places ${w.name} in every system prompt itself: a line names only its own sets of rules.`;
    case 'rulesNamedAsEnginePart':
      return `A set of rules is named ${w.name}, which is a part the engine places in every system prompt itself.`;
    case 'unusedRules':
      return `No line names the rules ${w.name}, so no agent would be given them.`;
    case 'valuesEmpty':
      return 'A choice has no values to choose from.';
    case 'noteNotByPeople':
      return `${w.block} is a note, but people do not write it.`;
    case 'noteIsAChoice':
      return `${w.block} is a note, but it is a choice, and a note is words to a cell.`;
    case 'noSuchBlock':
      return `The block ${w.block} is not declared in flow.toml.`;
    case 'notAnOutput':
      return `The cell must write ${w.block} every time, but ${w.block} is not among the blocks it may write.`;
    case 'describedNotRunOn':
      return `The cell says what the work is by ${w.block}, but ${w.block} does not run it.`;
    case 'takenNotRead':
      return `The cell says how it takes ${w.block}, but ${w.block} is not among the blocks it reads.`;
    case 'twoForms':
      return 'Name them one way: inputs, runs_on and given, or triggered_by and context.';
    case 'noSuchData':
      return `The flow names ${w.name} among the engine's own data, but no file of ../data/ defines ${w.name}.`;
    case 'definedTwice':
      return `${w.name} is defined here and again in ${w.also}.`;
    case 'noSuchTrigger':
      return `The cell names the trigger ${w.trigger}, which nothing in ../triggers/ defines.`;
    case 'noSuchDatum':
      return `The cell names the trigger ${w.trigger}, but the flow has no datum ${w.datum}.`;
    case 'triggerOnly':
      return `${w.block} stands only for a trigger and has no content: no cell is given it or writes it.`;
    case 'notLaidOut':
      return 'The cell names triggered_by and context, which only a flow laid out one thing to a file has.';
    case 'missingPlaceholder':
      return `This must hold ${w.placeholder}.`;
    case 'unknownReturnField':
      return `The engine's return has no field named "${w.field}".`;
    case 'unknownDetail':
      return `The engine fills no place named "${w.detail}".`;
    case 'readNotInContext':
      return `The trigger ${w.trigger} has the agent read ${w.block}, but ${w.block} is not in the cell's context.`;
    case 'notATrigger':
      return `The cell says what it does for ${w.trigger}, but ${w.trigger} is not among its triggers.`;
    case 'engineIsGiven':
      return `An engine's cell says how it is given ${w.block}, but it has no session.`;
    case 'runByNothing':
      return "Nothing runs this agent's cell, so it would never be entered.";
    case 'noKind':
      return `The cell is given ${w.block}, but does not say its kind.`;
    case 'saysNothing':
      return `The table for ${w.block} says nothing.`;
    case 'notRunOn':
      return `The cell says why ${w.block} runs it, but ${w.block} is not among what runs it.`;
    case 'notGiven':
      return `The cell says how it is given ${w.block}, but it is not given ${w.block}.`;
    case 'noWriter':
      return `Nothing writes ${w.block}: no cell of the order, no person and nothing outside.`;
    case 'noReader':
      return `Nothing reads ${w.block}: no cell, no condition, no test, and the page does not show it.`;
    case 'unknownSource':
      return `${w.block} is written by the source ${w.source}, which the engine does not have.`;
    case 'notAChoice':
      return `A condition or a test reads ${w.block}, which is not a choice.`;
    case 'notAValue':
      return `A condition or a test waits for ${w.block} to be ${w.is}, which is not one of its values.`;
    case 'conditionOnOwnOutput':
      return `The condition reads ${w.block}, which the cell itself writes and another writes too: the cell would decide by what it writes itself, beside another, whether it runs.`;
    case 'conditionNotWrittenBefore':
      return `The condition reads ${w.block}, which neither a cell before this one, people nor the outside write.`;
    case 'lastHasCondition':
      return 'The last cell has a condition, but a work ends when its last cell has a record, so it always runs.';
    case 'undeclaredTest':
      return `The test ${w.test} is not declared in flow.toml.`;
    case 'readsEmpty':
      return `The test ${w.test} reads no block.`;
    case 'unusedTest':
      return `No cell uses the test ${w.test}.`;
    case 'testOnOwnOutput':
      return `The test ${w.test} reads ${w.block}, which the cell itself writes: the cell would decide by what it writes itself whether the work waits before it, or, beside another writer, whether it runs.`;
    case 'testNotWrittenBefore':
      return `The test ${w.test} reads ${w.block}, which neither a cell before this one, people nor the outside write.`;
    case 'testWrittenAfter':
      return `The test ${w.test} reads ${w.block}, which ${w.cell}, a cell after this one, writes.`;
    case 'standingWrittenByTwo':
      return `The test ${w.test}, of what stands, reads ${w.block}, which ${w.cell} writes as well as a cell before it: a skipped cell's write would hide the other's word.`;
    case 'flowRefused': {
      const at = w.cell ? ` (${w.cell})` : '';
      const why = {
        empty: 'The flow has no cell.',
        twice: 'Two cells have the same name.',
        conditionOnOwnOutput: "A cell's condition reads a block the cell itself writes, and another writes it too.",
        lastHasCondition: 'The last cell has a condition.',
        noLimit: "A cell's limit is zero.",
        noSuchTest: 'A cell names a test the flow does not have.',
        unwritten: 'A test a cell uses reads a block that nothing writes.',
      }[w.reason];
      return `${why}${at}`;
    }
  }
}

/// A kind a cell gives a block in, as its file writes it; nothing where it says none.
export function inputKindWords(kind: InputKind | null): string {
  return kind === null ? '' : kind.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

/// How a cell is given a block: `given`, and its kind as the cell's file writes it.
export function givenWords(kind: InputKind | null): string {
  return kind ? `context ${inputKindWords(kind)}` : 'context';
}

/// Of each cell before the reader that writes a block, what decides whether it is there: the writer's `when`, or that
/// it need not write it (`must_write` false). Empty where nothing does.
export function thereWhen(flow: Flow, reader: string, block: Block): string {
  const at = flow.cells.findIndex((c) => c.id === reader);
  const parts: string[] = [];
  for (const id of block.writtenBy.cells) {
    const i = flow.cells.findIndex((c) => c.id === id);
    if (i < 0 || i >= at) continue;
    const writer = flow.cells[i];
    if (writer.condition !== null) parts.push(`${id} when ${conditionWords(writer.condition)}`);
    else if (!writer.mustWrite.includes(block.name)) parts.push(`${id} must_write false`);
  }
  return parts.join('; ');
}

/// What a cell's file says of a block, on one line, for the chart's column of words, each by its key: the `triggers`
/// that stand on it; `context`, its kind and its `why`; `outputs` or `must_write`; `when`; `requires`. Empty where the cell has
/// nothing to do with the block.
export function cellSays(cell: Cell, block: string): string {
  const parts: string[] = [];
  const input = cell.input[block];
  for (const trigger of cell.triggers.filter((t) => t.block === block)) {
    parts.push(`triggered_by ${trigger.name}`);
  }
  if (cell.given.includes(block)) {
    const how = givenWords(input?.given?.kind ?? null);
    const why = input?.given?.why;
    parts.push(why ? `${how}: ${why}` : how);
  }
  if (cell.outputs.includes(block)) {
    parts.push(cell.mustWrite.includes(block) ? 'must_write' : 'outputs');
  }
  const condition = cell.condition;
  if (
    condition !== null &&
    (condition.on === 'block' ? condition.block === block : condition.test.reads.includes(block))
  ) {
    parts.push(`when ${conditionWords(condition)}`);
  }
  for (const test of cell.preconditions) {
    if (test.reads.includes(block)) parts.push(`requires ${testWords(test)}`);
  }
  return parts.join('; ');
}

/// Who runs a cell: its line, or the engine.
export function runBy(cell: Cell): string {
  return cell.kind === 'agent' ? (cell.line ?? '') : 'engine';
}

/// What a cell's head on the chart has no room to say besides its title, each by its key: its line or `engine`, its
/// `when`, its `requires`; none of the last two where the cell has none.
export function headFacts(cell: Cell): {
  by: string;
  when: string | null;
  waits: string | null;
} {
  return {
    by: runBy(cell),
    when: cell.condition ? `when ${conditionWords(cell.condition)}` : null,
    waits: cell.preconditions.length > 0 ? `requires ${listed(cell.preconditions.map(testWords))}` : null,
  };
}

/// How long a text is, in characters, in words: `2,950 characters`.
export function charactersWords(n: number): string {
  return `${n.toLocaleString('en-US')} character${n === 1 ? '' : 's'}`;
}
