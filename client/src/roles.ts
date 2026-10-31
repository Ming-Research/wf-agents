// The colour each role is drawn in wherever the page shows it (its cells on a chart, its avatar and its bubbles), as the
// flow gives each line one: the lines are read once for every part of the page that shows a role, and a role is drawn
// in its colour through one lookup, the class of that colour (`role-blue`, roles.css), which sets the Primer tokens
// every part draws a role with; an agent with no colour of its own takes the page's neutral grey (`role-none`).

import { useMemo } from 'react';

import type { Colour, GraphLines } from './api';
import { useShared } from './live';

/// Each line's colour, by the line's name.
export type Colours = ReadonlyMap<string, Colour>;

/// The colours of the lines of every graph, as the interface gives them.
export function coloursOf(graphs: readonly GraphLines[]): Colours {
  return new Map(graphs.flatMap((g) => g.lines.map((l): [string, Colour] => [l.name, l.colour])));
}

/// The lines' colours, read once for every part of the page that shows a role; none until they are read.
export function useColours(): Colours {
  const graphs = useShared<GraphLines[]>('flows', '/api/v1/flows/lines');
  return useMemo(() => coloursOf(graphs ?? []), [graphs]);
}

/// The class a line's role is drawn in: its colour's, or the neutral grey's for no line, or a line with no colour (one
/// the flow no longer has, or any while the lines are not read yet).
export function roleClass(colours: Colours, line: string | null | undefined): string {
  const c = line != null ? colours.get(line) : undefined;
  return c !== undefined ? `role-${c}` : 'role-none';
}
