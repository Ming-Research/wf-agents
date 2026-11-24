// Made-up observations, for looking at the Observations view before the engine has kept any (`?fixtures`), and for its
// tests: two unread, of one return and of another work, and one read. Their works are the works fixtures' own.

import type { ObservationsPage } from '../observations-api';

/// When the fixtures' observations were kept: 2026-11-14 at 09:00, and a minute apart.
const AT = new Date(2026, 10, 14, 9, 0).getTime();
const MINUTE = 60 * 1000;

/// What every fixture has of an observation a run kept in a project of its own: no title, no tags, not taken in.
function of(itemTitle: string, work: number, item: number, cell: string, run: number, round: number) {
  return {
    title: null,
    tags: [],
    project: 'example',
    reportedBy: 'example',
    round: { work, item, itemTitle, cell, run, round },
    takenInBy: null,
    handled: false,
  };
}

export function fixtureObservations(): ObservationsPage {
  return {
    observations: [
      {
        number: 3,
        text:
          'The `query` tool answers a query that names a hidden column with the column\'s name in its words, so an ' +
          'agent learns the hidden column is there; `crates/carry/src/tools/query.rs` could say only that the query ' +
          'was refused.',
        ...of('Let agents read the store', 2, 2, 'R0', 4, 2),
        kept: AT + 2 * MINUTE,
        read: null,
      },
      {
        number: 2,
        text: 'The greeting has no test of an empty name.',
        ...of('Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        read: null,
      },
      {
        number: 1,
        text: 'The lexer drops the last token of a file with no newline at its end.',
        ...of('Greet the user', 1, 1, 'I1', 7, 1),
        kept: AT + MINUTE,
        read: AT + 5 * MINUTE,
      },
    ],
  };
}
