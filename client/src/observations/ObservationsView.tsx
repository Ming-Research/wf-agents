// The observations: what agents came across beyond their works, each a text of a return its run kept as it finished,
// the newest first. A row is the text, cut to three lines and whole on pointing at it, with where it came from on the line
// under it: its number, the work's run and round, which opens the work with that run chosen, the cell, the item and
// when it was kept. One the owner has not read is marked, and has the button that marks it read; one read is muted and
// says when. The list is read again on each change of a work, since a run that finishes keeps the observations of its
// return, and whenever the stream of changes opens again; a read that fails keeps the list as it was and says so.

import { useCallback, useState } from 'react';

import { usesFixtures } from '../mock-data';
import {
  type Observation,
  type ObservationsPage,
  loadObservations,
  readObservation,
} from '../observations-api';
import { Attr, Button } from '../parts';
import { useChanges, useReading } from '../works/source';
import { clock, fullTime } from '../works/words';

/// Where the view reads the observations from, and sends the owner's reading of one.
export interface ObservationsSource {
  load(): Promise<ObservationsPage>;
  read(number: number): Promise<unknown>;
}

/// The interface.
export const INTERFACE: ObservationsSource = { load: loadObservations, read: readObservation };

/// The fixtures, loaded only when asked for; the owner's reading of one is only logged.
export const FIXTURES: ObservationsSource = {
  load: async () => (await import('./fixtures')).fixtureObservations(),
  read: async (number) => {
    console.info(`observations/${number}/read`);
    return null;
  },
};

/// The source the page's address names.
function sourceOfPage(): ObservationsSource {
  return typeof window !== 'undefined' && usesFixtures(window.location.search) ? FIXTURES : INTERFACE;
}

export function ObservationsView({ source = sourceOfPage() }: { source?: ObservationsSource }) {
  const load = useCallback(() => source.load(), [source]);
  const { value: page, failed, read } = useReading(load);
  useChanges(read, read);
  if (!page) {
    return failed ? (
      <p className="message">The page could not read the observations: {failed}</p>
    ) : (
      <p className="message muted">Reading the observations</p>
    );
  }
  return <ObservationsList page={page} failed={failed} source={source} onRead={read} />;
}

/// The mark of an observation not yet read: a dot in the text's colour.
function UnreadMark() {
  return <span className="dot dot-unread" role="img" aria-label="unread" title="unread" />;
}

export function ObservationsList({ page, failed = null, source, onRead }: {
  page: ObservationsPage;
  /// Why the last read of the observations failed, where it did: what is shown is from before.
  failed?: string | null;
  source: ObservationsSource;
  /// Reads the observations again, once the owner's reading of one is kept.
  onRead: () => void;
}) {
  const { observations } = page;
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
          <ul className="legend" aria-label="What the marks of an observation mean">
            <li>
              <UnreadMark /> unread
            </li>
          </ul>
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
        {observations.length === 0 ? (
          <p className="message muted">No observation yet</p>
        ) : (
          <ul className="observations">
            {observations.map((o) => (
              <Row
                key={o.number}
                o={o}
                sending={sending.has(o.number)}
                onMarkRead={() => void markRead(o.number)}
              />
            ))}
          </ul>
        )}
      </main>
    </>
  );
}

/// One observation: its mark while unread, its text, where it came from, and at its right the button that marks it
/// read, or when it was read.
function Row({ o, sending, onMarkRead }: { o: Observation; sending: boolean; onMarkRead: () => void }) {
  const unread = o.read === null;
  return (
    <li className={unread ? 'observation' : 'observation observation-read'} data-observation={o.number}>
      <span className="observation-mark">{unread && <UnreadMark />}</span>
      <div className="observation-body">
        <p className="observation-text" title={o.text}>
          {o.text}
        </p>
        <p className="observation-facts">
          <span className="mono">O{o.number}</span>
          <a className="link mono" href={`#/works/${o.work}/runs/${o.run}`} title="Open the work with this run chosen">
            W{o.work} run {o.run} round {o.round}
          </a>
          <Attr name="cell" value={o.cell} />
          <span className="observation-item" title={o.title}>
            <span className="mono">#{o.item}</span> {o.title}
          </span>
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
