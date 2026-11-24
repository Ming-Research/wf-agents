// How a list of the owner's page is grouped by project, as the list of works and the observations are: a group for each
// project, each headed by its name outside its rows (parts.tsx, `ProjectGroup`).

/// One project's group: the project, none for what belongs to no project, and its rows in the list's order.
export interface ProjectGroup<T> {
  project: string | null;
  rows: T[];
}

/// The rows in a group for each project: first the projects in the order the owner gave them in the console (`order`),
/// those of them that have a group; then each other project the installation names, in the order given, which is the
/// order of their names, a project with no row among them; then each other project a row names, by name, as one the
/// installation no longer names; and last, where any row belongs to no project, the group of those. Each group keeps
/// its rows in the order given.
export function groupByProject<T>(
  names: readonly string[],
  rows: readonly T[],
  projectOf: (row: T) => string | null,
  order: readonly string[] = [],
): ProjectGroup<T>[] {
  const of = new Map<string | null, T[]>(names.map((name) => [name, []]));
  for (const row of rows) {
    const project = projectOf(row);
    const group = of.get(project);
    if (group === undefined) of.set(project, [row]);
    else group.push(row);
  }
  const others = [...of.keys()].filter((p): p is string => p !== null && !names.includes(p)).sort();
  const placed = new Set([...order.filter((p) => of.has(p)), ...names, ...others]);
  return [...placed, ...(of.has(null) ? [null] : [])].map((project) => ({
    project,
    rows: of.get(project) ?? [],
  }));
}
