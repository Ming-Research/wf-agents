// Made-up observations, for looking at the Observations view before the engine has kept any (`?fixtures`), and for its
// tests, each of the project of the works fixtures' item it is about (works/fixtures.ts), of three projects or as many
// as the address asks for. With three: in `example`, two not handled and one handled, taken in by the item of the round
// under way; in `Whitefoot`, one a round of a work of the works fixtures kept, taken in by the merged item; and in
// `wf-agents`, one an architect added, reported by `Whitefoot`, which the cancelled item took in, so that it is not
// handled.

import type { Observation, ObservationsPage } from '../observations-api';
import { fixtureItems } from '../works/fixtures';

/// When the fixtures' observations were kept: 2026-11-14 at 09:00, and a minute apart.
const AT = new Date(2026, 10, 14, 9, 0).getTime();
const MINUTE = 60 * 1000;

/// The items of the works fixtures each observation is about: the round under way's, the merged one's, the cancelled
/// one's, and that of the work at its limit of calls, of `Whitefoot` where there are two projects or more.
const ITEM = { running: 142, merged: 147, cancelled: 148, whitefoot: 143 } as const;

/// What every fixture has of an observation a run kept in a project of its own: no title, no tags, not read.
function of(project: string, itemTitle: string, work: number, item: number, cell: string, run: number, round: number) {
  return {
    title: null,
    tags: [],
    project,
    reportedBy: project,
    round: { work, item, itemTitle, cell, run, round },
    read: null,
  };
}

/// Not taken in by any item.
const OPEN = { takenInBy: null, handled: false };

export function fixtureObservations(projects?: number): ObservationsPage {
  const items = fixtureItems(projects).items;
  const projectOf = (item: number) => items.find((i) => i.number === item)?.project ?? 'wf-agents';
  const added: Observation = {
    number: 5,
    text: "The engine's log names a project's repository by its whole path, where the project's name would do.",
    title: null,
    tags: [],
    project: projectOf(ITEM.cancelled),
    reportedBy: projectOf(ITEM.whitefoot),
    round: null,
    kept: AT + 4 * MINUTE,
    read: null,
    // An item whose work was cancelled took it in: it is not handled.
    takenInBy: ITEM.cancelled,
    handled: false,
  };
  return {
    observations: [
      added,
      {
        number: 4,
        text: 'The list of works reads every item again for a change of one.',
        ...of(projectOf(ITEM.whitefoot), 'List every work', 43, ITEM.whitefoot, 'plan', 1, 2),
        kept: AT + 3 * MINUTE,
        takenInBy: ITEM.merged,
        handled: true,
      },
      {
        number: 3,
        text:
          'The `query` tool answers a query that names a hidden column with the column\'s name in its words, so an ' +
          'agent learns the hidden column is there; `crates/carry/src/tools/query.rs` could say only that the query ' +
          'was refused.',
        ...of(projectOf(ITEM.running), 'Let agents read the store', 2, 2, 'R0', 4, 2),
        kept: AT + 2 * MINUTE,
        ...OPEN,
      },
      {
        number: 2,
        text: 'The greeting has no test of an empty name.',
        ...of(projectOf(ITEM.running), 'Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        takenInBy: ITEM.running,
        handled: true,
      },
      {
        number: 1,
        text: 'The lexer drops the last token of a file with no newline at its end.',
        ...of(projectOf(ITEM.running), 'Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        ...OPEN,
      },
    ],
  };
}
