// The observations: what agents came across beyond their works, each a text of a return its run kept as it finished,
// and what an architect added. They stand in a group for each project, in order of the projects' names (projects.ts),
// each observation in the group of the project it is of, the newest first. A group's head stands outside its rows, as
// the works' groups' heads do: the project's name, which folds the group away and opens it again, with how many
// observations it holds and how many of them the owner has not read (parts.tsx, `ProjectGroup`); which groups are
// folded is kept in the browser. A row is the text, cut to three lines and whole on pointing at it, the number of an
// item in it a link to the item (`itemNumbers.tsx`), with where it came from on the line under it: its number; for one a
// run kept, the work, which opens it with that run and round chosen, the cell and the item; where another project than
// its own reported it, that project; and when it was kept. One the owner has not read is marked, and has the button
// that marks it read; one read is muted and says when. The list is read again on each change the stream tells of,
// since a run that finishes keeps the observations of its return and an architect adds and changes them, and whenever
// the stream of changes opens again, the projects with it; a read that fails keeps the list as it was and says so.

import { useCallback, useMemo, useState } from 'react';

import { linkItemNumbers, useItemTitles } from '../itemNumbers';
import { CHOICE_KEYS, useKeptFoldedGroups } from '../kept';
import { Legend, UnreadMark } from '../marks';
import { projectsAsked, usesFixtures } from '../mock-data';
import {
  type Observation,
  type ObservationsPage,
  loadObservations,
  readObservation,
} from '../observations-api';
import { Attr, Button, ProjectGroup, RoundLink } from '../parts';
import { groupByProject } from '../projects';
import { useChanges, useProjects, useReading } from '../works/source';
import { clock, fullTime, itemName } from '../works/words';

/// Where the view reads the observations from, and sends the owner's reading of one.
export interface ObservationsSource {
  load(): Promise<ObservationsPage>;
  read(number: number): Promise<unknown>;
}

/// The interface.
export const INTERFACE: ObservationsSource = { load: loadObservations, read: readObservation };

/// The fixtures, of as many projects as the address asks for or else of their own number of them, loaded only when
/// asked for; the owner's reading of one is only logged.
export const fixturesOf = (projects: number | null): ObservationsSource => ({
  load: async () => (await import('./fixtures')).fixtureObservations(projects ?? undefined),
  read: async (number) => {
    console.info(`observations/${number}/read`);
    return null;
  },
});

/// The source the page's address names.
function sourceOfPage(): ObservationsSource {
  return typeof window !== 'undefined' && usesFixtures(window.location.search)
    ? fixturesOf(projectsAsked(window.location.search))
    : INTERFACE;
}

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
  return <ObservationsList page={page} projects={projects} failed={failed} source={source} onRead={read} />;
}

export function ObservationsList({ page, projects, failed = null, source, onRead }: {
  page: ObservationsPage;
  /// The projects the installation names, in order of their names.
  projects: string[];
  /// Why the last read of the observations failed, where it did: what is shown is from before.
  failed?: string | null;
  source: ObservationsSource;
  /// Reads the observations again, once the owner's reading of one is kept.
  onRead: () => void;
}) {
  const { observations } = page;
  const groups = useMemo(() => groupByProject(projects, observations, (o) => o.project), [projects, observations]);
  const [folded, turn] = useKeptFoldedGroups(CHOICE_KEYS.observationsFolded);
  // The observations whose reading is on its way, and why the last that was refused was.
  const [sending, setSending] = useState<ReadonlySet<number>>(new Set());
  const [refused, setRefused] = useState<string | null>(null);
  const markRead = async (number: number) => {
    setSending((s) => new Set(s).add(number));
    setRefused(null);
    try {
      await source.read(number);
      onRead();
    } catch (e) {
      setRefused(`O${number} could not be marked read: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSending((s) => {
        const left = new Set(s);
        left.delete(number);
        return left;
      });
    }
  };
  return (
    <>
      <div className="view-line">
        <span className="view-name">Observations</span>
        <span className="view-facts">{observations.length} observations</span>
        <span className="view-end">
          <Legend marks={['unread']} />
        </span>
      </div>
      {failed !== null && (
        <p className="read-failed" role="alert">
          The observations could not be read again: {failed}. What is shown is from before.
        </p>
      )}
      {refused !== null && (
        <p className="read-failed" role="alert">
          {refused}
        </p>
      )}
      <main className="list-view">
        {groups.length === 0 ? (
          <p className="message muted">No observation yet</p>
        ) : (
          groups.map(({ project, rows }) => {
            const unread = rows.filter((o) => o.read === null).length;
            return (
              <ProjectGroup
                key={JSON.stringify(project)}
                project={project}
                what="observations"
                count={rows.length}
                countTitle={`${rows.length} ${rows.length === 1 ? 'observation' : 'observations'}`}
                note={unread > 0 ? `${unread} unread` : undefined}
                open={!folded.has(project)}
                onToggle={() => turn(project)}
              >
                {rows.length === 0 ? (
                  <p className="message muted">No observation of project {project} yet</p>
                ) : (
                  <ul className="observations">
                    {rows.map((o) => (
                      <Row
                        key={o.number}
                        o={o}
                        sending={sending.has(o.number)}
                        onMarkRead={() => void markRead(o.number)}
                      />
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

/// One observation: its mark while unread, its text, where it came from, and at its right the button that marks it
/// read, or when it was read. Where it came from leaves out the project it is of, which its group's head says, and
/// names the project that reported it only where that is another.
function Row({ o, sending, onMarkRead }: { o: Observation; sending: boolean; onMarkRead: () => void }) {
  const unread = o.read === null;
  const titles = useItemTitles();
  return (
    <li className={unread ? 'observation' : 'observation observation-read'} data-observation={o.number}>
      <span className="observation-mark">{unread && <UnreadMark />}</span>
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
          <span title={fullTime(o.kept)}>
            <Attr name="kept" value={clock(o.kept)} />
          </span>
        </p>
      </div>
      <span className="observation-end">
        {o.read === null ? (
          <Button disabled={sending} title={`Mark O${o.number} read`} onClick={onMarkRead}>
            Mark read
          </Button>
        ) : (
          <span title={fullTime(o.read)}>
            <Attr name="read" value={clock(o.read)} />
          </span>
        )}
      </span>
    </li>
  );
}
