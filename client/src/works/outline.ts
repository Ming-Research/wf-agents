// The version of its flow a work runs on, read from its outline (`Outline`) as the parts of the flow's chart read a
// flow, so that a work's history draws its heads and its lines with them: each cell with its title, who runs it and
// what it is given, watches and reads; the lines and the groups. What the outline does not keep is not drawn: no cell
// has a task, a condition, an output or limits, and the version has no blocks, tests or tools.

import type { Cell, Flow } from '../api';
import type { OutlineCell, WorkPage } from '../works-api';

/// The limits of a cell or a flow the outline does not keep.
const NO_LIMITS = { runs: 0, returns: 0, calls: 0 };

/// A cell of the version as a chart's head draws it. A cell the version does not have, or any of a version not kept,
/// says so for its title, and is taken for an agent's where a try of its rounds was recorded, in the line of the session
/// the first such round ran in, as the work's usage takes it (`used.ts`).
function cellOf(c: OutlineCell, page: Pick<WorkPage, 'outline' | 'steps'>): Cell {
  const known = c.title !== null;
  const tried = page.steps
    .flatMap((s) => (s.kind === 'run' && s.cell === c.id ? s.rounds : []))
    .find((r) => r.tries.length > 0);
  const agent = known ? c.agent : tried !== undefined;
  return {
    id: c.id,
    title: c.title ?? (page.outline.kept ? 'not in this version' : 'of a version not kept'),
    kind: agent ? 'agent' : 'engine',
    line: known ? c.line : (tried?.session?.line ?? null),
    task: null,
    taskFile: null,
    act: null,
    effort: null,
    takesMain: false,
    when: [],
    requires: [],
    watches: c.watches,
    reads: c.reads,
    given: c.given,
    outputs: [],
    mustWrite: [],
    tools: [],
    limits: NO_LIMITS,
  };
}

/// The version of its flow a work runs on, as its outline has it.
export function versionFlow(page: Pick<WorkPage, 'outline' | 'steps' | 'summary'>): Flow {
  return {
    name: page.summary.flow,
    limits: NO_LIMITS,
    lines: [],
    tests: [],
    tools: [],
    groups: page.outline.groups,
    card: page.outline.card,
    everyAgent: { reads: [], kinds: {} },
    cells: page.outline.cells.map((c) => cellOf(c, page)),
    blocks: [],
  };
}
