// The observations: what agents came across beyond their works, each a text of a return its run kept as it finished,
// and what an architect added. They stand in a group for each project, in the order the owner gave the projects in the
// console and then in order of their names (projects.ts), read again at each drawing, each observation in the group of
// the project it is of, the newest first. A group's head stands outside its rows, as the works' groups' heads do: the
// project's name, which folds the group away and opens it again, with how many observations it holds and how many of
// them are still open (parts.tsx, `ProjectGroup`); which groups are folded is kept in the browser. A row is the text, cut
// to three lines and whole on pointing at it, the number of an item in it a link to the item (`itemNumbers.tsx`), with
// where it came from on the line under it: its number; for one a run kept, the work, which opens it with that run and
// round chosen, the cell and the item; where another project than its own reported it, that project; the item that
// took it in, where one did; and when it was kept. At its right it says whether it is handled: an item whose work is not
// cancelled took it in. The list is read again on each change the stream tells of, since a run that finishes keeps the
// observations of its return and an architect adds and changes them, and whenever the stream of changes opens again,
// the projects with it; a read that fails keeps the list as it was and says so.

import { useCallback, useMemo } from 'react';

import { ItemLink, linkItemNumbers, useItemTitles } from '../itemNumbers';
import { CHOICE_KEYS, useKeptFoldedGroups, useKeptProjectOrder } from '../kept';
import { projectsAsked, usesFixtures } from '../mock-data';
import { type Observation, type ObservationsPage, loadObservations } from '../observations-api';
import { Attr, ProjectGroup, RoundLink } from '../parts';
import { groupByProject } from '../projects';
import { useChanges, useProjects, useReading } from '../works/source';
import { clock, fullTime, itemName } from '../works/words';

/// Where the view reads the observations from.
export interface ObservationsSource {
  load(): Promise<ObservationsPage>;
}

/// The interface.
export const INTERFACE: ObservationsSource = { load: loadObservations };

/// The fixtures, of as many projects as the address asks for or else of their own number of them, loaded only when
/// asked for.
export const fixturesOf = (projects: number | null): ObservationsSource => ({
  load: async () => (await import('./fixtures')).fixtureObservations(projects ?? undefined),
});

/// The source the page's address names.
function sourceOfPage(): ObservationsSource {
  return typeof window !== 'undefined' && usesFixtures(window.location.search)
    ? fixturesOf(projectsAsked(window.location.search))
    : INTERFACE;
}

/// Whether an observation is handled, drawn with the dot and the word of an item's standing, as the item's window and
/// the console's search draw it (console/ItemWindow.tsx, `DOT` and `STANDING_WORDS`): the green dot of what is done
/// for one handled, and the dot and the word of what is open for one not handled yet; each says on pointing at it what
/// it means.
const HANDLED = {
  handled: { dot: 'dot dot-added', word: 'Handled', title: 'An item whose work is not cancelled took it in' },
  open: { dot: 'dot dot-running', word: 'Open', title: 'No item took it in yet, or only one whose work was cancelled' },
} as const;

export function ObservationsView({ source: given }: { source?: ObservationsSource }) {
  // The page's own source is made once for the view, so that it is not read again for each drawing.
  const source = useMemo(() => given ?? sourceOfPage(), [given]);
  const load = useCallback(() => source.load(), [source]);
  const { value: page, failed, read } = useReading(load);
  useChanges(read, read);
  // Where the projects cannot be read, or are not yet, the groups are those of the projects the observations are of.
  const projects = useProjects();
  if (!page) {
    return failed ? (
      <p className="message">The page could not read the observations: {failed}</p>
    ) : (
      <p className="message muted">Reading the observations</p>
    );
  }
  return <ObservationsList page={page} projects={projects} failed={failed} />;
}

export function ObservationsList({ page, projects, failed = null }: {
  page: ObservationsPage;
  /// The projects the installation names, in order of their names.
  projects: string[];
  /// Why the last read of the observations failed, where it did: what is shown is from before.
  failed?: string | null;
}) {
  const { observations } = page;
  const order = useKeptProjectOrder();
  const groups = useMemo(
    () => groupByProject(projects, observations, (o) => o.project, order),
    [projects, observations, order],
  );
  const [folded, turn] = useKeptFoldedGroups(CHOICE_KEYS.observationsFolded);
  return (
    <>
      <div className="view-line">
        <span className="view-name">Observations</span>
        <span className="view-facts">{observations.length} observations</span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The observations could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      <main className="list-view">
        {groups.length === 0 ? (
          <p className="message muted">No observation yet</p>
        ) : (
          groups.map(({ project, rows }) => {
            const open = rows.filter((o) => !o.handled).length;
            return (
              <ProjectGroup
                key={JSON.stringify(project)}
                project={project}
                what="observations"
                count={rows.length}
                countTitle={`${rows.length} ${rows.length === 1 ? 'observation' : 'observations'}`}
                note={open > 0 ? `${open} open` : undefined}
                noteTitle={`${open} not handled yet`}
                open={!folded.has(project)}
                onToggle={() => turn(project)}
              >
                {rows.length === 0 ? (
                  <p className="message muted">No observation of project {project} yet</p>
                ) : (
                  <ul className="observations">
                    {rows.map((o) => (
                      <Row key={o.number} o={o} />
                    ))}
                  </ul>
                )}
              </ProjectGroup>
            );
          })
        )}
      </main>
    </>
  );
}

/// One observation: its text, where it came from, and at its right whether it is handled. Where it came from leaves
/// out the project it is of, which its group's head says, and names the project that reported it only where that is
/// another. The item that took it in leads to it; one whose work was cancelled, which leaves the observation open, has
/// its title grey and struck through, as a cancelled item's title is on the console's cards.
function Row({ o }: { o: Observation }) {
  const titles = useItemTitles();
  const standing = o.handled ? HANDLED.handled : HANDLED.open;
  return (
    <li className="observation" data-observation={o.number}>
      <div className="observation-body">
        <p className="observation-text" title={o.text}>
          {linkItemNumbers(o.text, titles)}
        </p>
        <p className="observation-facts">
          <span className="mono">O{o.number}</span>
          {o.round !== null && (
            <>
              <RoundLink work={o.round.work} run={o.round.run} round={o.round.round} />
              <Attr name="cell" value={o.round.cell} />
              <span className="observation-item" title={o.round.itemTitle}>
                <span className="mono">{itemName(o.round.item)}</span> {o.round.itemTitle}
              </span>
            </>
          )}
          {o.reportedBy !== null && o.reportedBy !== o.project && <Attr name="reported by" value={o.reportedBy} />}
          {o.takenInBy !== null && (
            <span className={o.handled ? 'observation-item' : 'observation-item observation-taken-cancelled'}>
              <span className="muted mono">taken in by</span> <ItemLink item={o.takenInBy} title={titles.get(o.takenInBy)} />
              {titles.has(o.takenInBy) && (
                <>
                  {' '}
                  <span className="observation-taken-title" title={o.handled ? undefined : 'Its work was cancelled'}>
                    {titles.get(o.takenInBy)}
                  </span>
                </>
              )}
            </span>
          )}
          <span title={fullTime(o.kept)}>
            <Attr name="kept" value={clock(o.kept)} />
          </span>
        </p>
      </div>
      <span className="observation-end" title={standing.title}>
        <span className={standing.dot} aria-hidden="true" />
        {standing.word}
      </span>
    </li>
  );
}
