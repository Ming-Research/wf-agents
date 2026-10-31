// The colour each role is drawn in wherever the page shows it (its cells on a chart, its avatar and its bubbles), as the
// flow gives each line one: the lines are read once, as the page starts, for every part of the page that shows a role,
// and a role is drawn in its colour through one lookup, the class of that colour (`role-blue`, roles.css), which sets
// the Primer tokens every part draws a role with; an agent with no colour of its own, or any while the lines are not
// read yet, takes the page's neutral grey (`role-none`).

import { useMemo } from 'react';

import type { Colour, GraphLines } from './api';
import { readAhead, useShared } from './live';

/// Where the lines are read from, and what names them changed: a new flow version may be a new build's flow.
const LINES_KEY = 'flows';
const LINES_PATH = '/api/v1/flows/lines';

/// Each line's colour, by the line's name.
export type Colours = ReadonlyMap<string, Colour>;

/// The colours of the lines of every graph, as the interface gives them.
export function coloursOf(graphs: readonly GraphLines[]): Colours {
  return new Map(graphs.flatMap((g) => g.lines.map((l): [string, Colour] => [l.name, l.colour])));
}

/// The lines' colours, read once for every part of the page that shows a role; none until they are read.
export function useColours(): Colours {
  const graphs = useShared<GraphLines[]>(LINES_KEY, LINES_PATH);
  return useMemo(() => coloursOf(graphs ?? []), [graphs]);
}

/// Reads the lines' colours as the page starts, before any part that shows a role is drawn, so that a bubble or a cell
/// is drawn in its colour from the first and not in the grey before it.
export function readColours(): void {
  readAhead(LINES_KEY, LINES_PATH);
}

/// The class a line's role is drawn in: its colour's, or the neutral grey's for no line, or a line with no colour (one
/// the flow no longer has, or any while the lines are not read yet).
export function roleClass(colours: Colours, line: string | null | undefined): string {
  const c = line != null ? colours.get(line) : undefined;
  return c !== undefined ? `role-${c}` : 'role-none';
}
