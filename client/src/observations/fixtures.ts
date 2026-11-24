// Made-up observations, for looking at the Observations view before the engine has kept any (`?fixtures`), and for its
// tests, of the works fixtures' projects (works/fixtures.ts), three of them or as many as the address asks for: in
// `example`, two unread, of one return and of another work, and one read; in `Whitefoot`, one read, kept by a round of
// a work of the works fixtures; and in `wf-agents`, one unread that an architect added, reported by `Whitefoot`. An
// observation whose project the fixtures do not have is of `wf-agents`, which they always have.

import type { Observation, ObservationsPage } from '../observations-api';
import { fixtureProjectNames } from '../works/fixtures';

/// When the fixtures' observations were kept: 2026-11-14 at 09:00, and a minute apart.
const AT = new Date(2026, 10, 14, 9, 0).getTime();
const MINUTE = 60 * 1000;

/// What every fixture has of an observation a run kept in a project of its own: no title, no tags, not taken in.
function of(project: string, itemTitle: string, work: number, item: number, cell: string, run: number, round: number) {
  return {
    title: null,
    tags: [],
    project,
    reportedBy: project,
    round: { work, item, itemTitle, cell, run, round },
    takenInBy: null,
    handled: false,
  };
}

export function fixtureObservations(projects?: number): ObservationsPage {
  const names = fixtureProjectNames(projects);
  const project = (name: string) => (names.includes(name) ? name : 'wf-agents');
  const added: Observation = {
    number: 5,
    text: "The engine's log names a project's repository by its whole path, where the project's name would do.",
    title: null,
    tags: [],
    project: 'wf-agents',
    reportedBy: project('Whitefoot'),
    round: null,
    kept: AT + 4 * MINUTE,
    read: null,
    takenInBy: null,
    handled: false,
  };
  return {
    observations: [
      added,
      {
        number: 4,
        text: 'The list of works reads every item again for a change of one.',
        ...of(project('Whitefoot'), 'List every work', 43, 143, 'plan', 1, 2),
        kept: AT + 3 * MINUTE,
        read: AT + 6 * MINUTE,
      },
      {
        number: 3,
        text:
          'The `query` tool answers a query that names a hidden column with the column\'s name in its words, so an ' +
          'agent learns the hidden column is there; `crates/carry/src/tools/query.rs` could say only that the query ' +
          'was refused.',
        ...of(project('example'), 'Let agents read the store', 2, 2, 'R0', 4, 2),
        kept: AT + 2 * MINUTE,
        read: null,
      },
      {
        number: 2,
        text: 'The greeting has no test of an empty name.',
        ...of(project('example'), 'Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        read: null,
      },
      {
        number: 1,
        text: 'The lexer drops the last token of a file with no newline at its end.',
        ...of(project('example'), 'Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        read: AT + 5 * MINUTE,
      },
    ],
  };
}
