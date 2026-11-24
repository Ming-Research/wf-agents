// The list of the projects under the console's map: each project's name after the key that shows its tree, 1 to 9 by
// its place in the list, the one shown in the tint a row chosen has, and after the name what waits for the owner in
// it, counted as the bar counts it beside the console's tab. A click on a name shows its tree as its key does, and a
// name dragged to another place puts the list in that order, which the console keeps in the browser.

import { useState } from 'react';

import { aheadCount as aheadWords, waitCount } from '../works/words';

/// What the list says: of itself, and of the key before each name.
export const PROJECTS_SAYS = {
  list: 'The projects: a press of its key or a click shows its tree; drag a name to change the order',
  key: (n: number) => `Press ${n} to show this project's tree`,
} as const;

export function ProjectList({
  projects,
  counts,
  shown,
  onShow,
  onOrder,
}: {
  /// The projects, in the list's order.
  projects: string[];
  /// Of each project, how many things wait for the owner in it, and how many questions the owner may answer ahead.
  counts: ReadonlyMap<string, { waiting: number; ahead: number }>;
  /// The project whose tree the console shows.
  shown: string | null;
  onShow: (name: string) => void;
  /// Told the projects in their new order while a name is dragged.
  onOrder: (names: string[]) => void;
}) {
  const [dragging, setDragging] = useState<string | null>(null);
  if (projects.length === 0) return null;
  const move = (name: string, to: number) => {
    const rest = projects.filter((p) => p !== name);
    rest.splice(to, 0, name);
    onOrder(rest);
  };
  return (
    <ol className="console-projects" aria-label="The projects" title={PROJECTS_SAYS.list}>
      {projects.map((name, k) => (
        <li
          key={name}
          className={name === dragging ? 'console-project dragging' : 'console-project'}
          draggable
          onDragStart={(e) => {
            setDragging(name);
            e.dataTransfer.effectAllowed = 'move';
            e.dataTransfer.setData('text/plain', name);
          }}
          onDragOver={(e) => {
            if (dragging === null) return;
            e.preventDefault();
            if (dragging !== name) move(dragging, k);
          }}
          onDrop={(e) => e.preventDefault()}
          onDragEnd={() => setDragging(null)}
        >
          <button
            type="button"
            className={name === shown ? 'console-project-name shown' : 'console-project-name'}
            aria-current={name === shown ? 'true' : undefined}
            onClick={() => onShow(name)}
          >
            {k < 9 ? (
              <kbd className="key" title={PROJECTS_SAYS.key(k + 1)}>
                {k + 1}
              </kbd>
            ) : (
              <span className="key-none" />
            )}
            <span className="console-project-title">{name}</span>
            <Counts {...(counts.get(name) ?? { waiting: 0, ahead: 0 })} />
          </button>
        </li>
      ))}
    </ol>
  );
}

/// What waits for the owner in a project, as the bar draws it beside a view's name: the questions to answer ahead in
/// orange, left of the things that wait, in red; each while it is above nought.
function Counts({ waiting, ahead }: { waiting: number; ahead: number }) {
  if (waiting === 0 && ahead === 0) return null;
  return (
    <span className="console-project-counts">
      {ahead > 0 && (
        <span className="bar-count ahead" title={aheadWords(ahead)}>
          {ahead}
        </span>
      )}
      {waiting > 0 && (
        <span className="bar-count" title={waitCount(waiting)}>
          {waiting}
        </span>
      )}
    </span>
  );
}
