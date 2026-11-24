import { describe, expect, it } from 'vitest';

import { groupByProject } from './projects';

type Row = { n: number; project: string | null };
const row = (n: number, project: string | null): Row => ({ n, project });
const grouped = (names: string[], rows: Row[]) =>
  groupByProject(names, rows, (r) => r.project).map((g) => [g.project, g.rows.map((r) => r.n)]);

describe('a list grouped by project', () => {
  it("is a group for each project the installation names, in the order given, each with its rows in the list's order", () => {
    const rows = [row(5, 'wf-agents'), row(4, 'Whitefoot'), row(3, 'wf-agents'), row(2, 'example'), row(1, 'Whitefoot')];
    expect(grouped(['Whitefoot', 'example', 'wf-agents'], rows)).toEqual([
      ['Whitefoot', [4, 1]],
      ['example', [2]],
      ['wf-agents', [5, 3]],
    ]);
  });

  it('keeps a group for a project with no row, so that every project the installation names is shown', () => {
    expect(grouped(['Whitefoot', 'example'], [row(1, 'Whitefoot')])).toEqual([
      ['Whitefoot', [1]],
      ['example', []],
    ]);
  });

  it('puts a project the installation does not name after those it does, by name, and the rows of no project last', () => {
    const rows = [row(4, null), row(3, 'zeta'), row(2, 'alpha'), row(1, 'wf-agents')];
    expect(grouped(['wf-agents'], rows)).toEqual([
      ['wf-agents', [1]],
      ['alpha', [2]],
      ['zeta', [3]],
      [null, [4]],
    ]);
  });

  it('groups by the projects the rows name where the installation names none, as before its projects are read', () => {
    expect(grouped([], [row(2, 'wf-agents'), row(1, 'Whitefoot')])).toEqual([
      ['Whitefoot', [1]],
      ['wf-agents', [2]],
    ]);
    expect(grouped([], [])).toEqual([]);
  });
});
