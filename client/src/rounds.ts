// The window of an architect's rounds its page lists, across all its works as one list: what is read of it (the
// newest, those before or after the rounds listed, or a round with those around it), how what comes back joins what is
// listed, what is read again when its rounds change, and where its list is scrolled to as rounds join it or one is
// chosen. A work's page lists every round of its one work, read with the work, and scrolls its list the same way.

import { useCallback, useEffect, useRef, useState } from 'react';

import type { ArchitectRounds, RoundSummary } from './api';
import { roundName } from './format';
import { getJson } from './http';
import { useVersion } from './live';
import { forProject, type RoundKey, sameRound } from './route';

/// The rounds listed, oldest first, and whether older and newer ones are there to read.
export type RoundsWindow = { rounds: RoundSummary[]; older: boolean; newer: boolean };

/// How many rounds each read of the window takes: more than a tall panel shows, so that the list scrolls before it
/// reads more.
export const WINDOW_STEP = 100;

/// What is read of the window: the newest rounds; those before a round, the oldest listed; those after a round; or a
/// round with those around it.
export type WindowRead =
  | { kind: 'newest' }
  | { kind: 'older'; before: RoundKey }
  | { kind: 'newer'; after: RoundKey }
  | { kind: 'around'; round: RoundKey };

/// The interface's path that reads `read` of a project's architect's rounds.
export function windowPath(project: string, read: WindowRead): string {
  const at = (name: string, k: RoundKey) => `&${name}=${roundName(k.work, k.n)}`;
  const which =
    read.kind === 'older'
      ? at('before', read.before)
      : read.kind === 'newer'
        ? at('after', read.after)
        : read.kind === 'around'
          ? at('around', read.round)
          : '';
  return `${forProject('/api/v1/architect/rounds', project)}${which}&limit=${WINDOW_STEP}`;
}

const keyOf = (r: RoundSummary): RoundKey => ({ work: r.work, n: r.n });

/// The window once a read came back: the newest, or a round with those around it, in place of what was listed; older
/// rounds before those listed; newer rounds in place of those listed after the round they were read after.
export function merged(was: RoundsWindow | null, read: WindowRead, got: ArchitectRounds): RoundsWindow {
  if (was === null || read.kind === 'newest' || read.kind === 'around') {
    return { rounds: got.rounds, older: got.older, newer: got.newer };
  }
  if (read.kind === 'older') {
    const kept = was.rounds.filter((r) => !got.rounds.some((x) => sameRound(keyOf(x), keyOf(r))));
    return { rounds: [...got.rounds, ...kept], older: got.older, newer: was.newer };
  }
  const at = was.rounds.findIndex((r) => sameRound(keyOf(r), read.after));
  const kept = at >= 0 ? was.rounds.slice(0, at + 1) : was.rounds;
  return { rounds: [...kept, ...got.rounds], older: was.older, newer: got.newer };
}

/// What is read again when the architect's rounds changed: nothing while newer rounds are left unread, since the
/// newest are not listed; else those after the round before the first round of the newest work still open, which may
/// have ended since, or after the newest listed; the newest again when nothing is listed before that open round.
export function refreshRead(w: RoundsWindow): WindowRead | null {
  if (w.newer) return null;
  const last = w.rounds[w.rounds.length - 1];
  if (last === undefined) return { kind: 'newest' };
  const open = w.rounds.findIndex((r) => r.work === last.work && r.outcome === 'running');
  if (open === 0) return { kind: 'newest' };
  const from = open > 0 ? w.rounds[open - 1] : last;
  return { kind: 'newer', after: keyOf(from) };
}

/// What the window reads as the page opens or a round is chosen: the newest when nothing is listed and no round is
/// asked for; a round asked for with those around it when it is not listed; else nothing, the round listed being
/// chosen where it is.
export function openRead(w: RoundsWindow | null, round: RoundKey | null): WindowRead | null {
  if (round !== null) return w !== null && w.rounds.some((r) => sameRound(keyOf(r), round)) ? null : { kind: 'around', round };
  return w === null ? { kind: 'newest' } : null;
}

/// A project's architect's rounds as its page lists them: opened on the newest, or around the round asked for, which
/// is read around afresh whenever it is not listed; more read before or after the rounds listed as the list scrolls to
/// its ends (`more`), and the newest read again as the architect's rounds change.
export function useRoundsWindow(
  project: string,
  round: RoundKey | null,
): { w: RoundsWindow | null; error: string | null; loading: 'older' | 'newer' | null; more: (end: 'older' | 'newer') => void } {
  const [held, setHeld] = useState<{ project: string; w: RoundsWindow } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState<'older' | 'newer' | null>(null);
  const w = held !== null && held.project === project ? held.w : null;
  // The window as last read, for a read that starts before the page draws it.
  const now = useRef<{ project: string; w: RoundsWindow } | null>(held);
  // Each opening of the window starts a generation; what an earlier one read comes back too late and is dropped.
  const generation = useRef(0);
  const busy = useRef(false);
  const again = useRef(false);
  const readRef = useRef<(read: WindowRead, opens: boolean) => void>(() => {});
  const read = useCallback(
    (r: WindowRead, opens: boolean) => {
      if (opens) {
        generation.current += 1;
        again.current = false;
      } else if (busy.current) {
        // What changed while a read was under way is read once it is done.
        if (r.kind !== 'older') again.current = true;
        return;
      }
      const at = generation.current;
      busy.current = true;
      setLoading(r.kind === 'older' || r.kind === 'newer' ? r.kind : null);
      const done = () => {
        busy.current = false;
        setLoading(null);
      };
      getJson<ArchitectRounds>(windowPath(project, r)).then(
        (got) => {
          if (generation.current !== at) return;
          const was = now.current !== null && now.current.project === project ? now.current.w : null;
          const next = { project, w: merged(was, r, got) };
          now.current = next;
          setHeld(next);
          setError(null);
          done();
          if (again.current) {
            again.current = false;
            const fresh = refreshRead(next.w);
            if (fresh !== null) readRef.current(fresh, false);
          }
        },
        (e: unknown) => {
          if (generation.current !== at) return;
          done();
          setError(e instanceof Error ? e.message : String(e));
        },
      );
    },
    [project],
  );
  readRef.current = read;
  // Opens the window on the newest, or around a round asked for that is not listed.
  const opening = openRead(w, round);
  const opens = opening === null ? null : opening.kind === 'around' ? roundName(opening.round.work, opening.round.n) : 'newest';
  useEffect(() => {
    if (opening !== null) read(opening, true);
    // Only when what is to be opened changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project, opens]);
  // The architect's rounds changed: the newest listed are read again.
  const changed = useVersion('architect');
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const was = now.current !== null && now.current.project === project ? now.current.w : null;
    const fresh = was !== null ? refreshRead(was) : null;
    if (fresh !== null) read(fresh, false);
    // Only when the architect's rounds changed.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [changed]);
  const more = useCallback(
    (end: 'older' | 'newer') => {
      const was = now.current !== null && now.current.project === project ? now.current.w : null;
      if (was === null || was.rounds.length === 0) return;
      if (end === 'older' && was.older) read({ kind: 'older', before: keyOf(was.rounds[0]) }, false);
      if (end === 'newer' && was.newer) read({ kind: 'newer', after: keyOf(was.rounds[was.rounds.length - 1]) }, false);
    },
    [project, read],
  );
  return { w, error, loading, more };
}

/// How near an end of the list, in pixels, its scrolling reads more: about two rows, so that the rows read join it
/// before the owner reaches its end.
export const NEAR_END = 64;

/// The ends of a list scrolled so that more is read there: its top while it is within `NEAR_END` of it and newer
/// rounds are left unread, its bottom likewise for older; none while the list is not drawn (no height), as in a
/// minimised window.
export function endsNear(
  s: { scrollTop: number; scrollHeight: number; clientHeight: number },
  more: { older: boolean; newer: boolean },
): Array<'older' | 'newer'> {
  if (s.clientHeight <= 0) return [];
  const out: Array<'older' | 'newer'> = [];
  if (more.newer && s.scrollTop <= NEAR_END) out.push('newer');
  if (more.older && s.scrollHeight - s.scrollTop - s.clientHeight <= NEAR_END) out.push('older');
  return out;
}

/// Where a list scrolls to bring a row to its middle: the row's top, from the top of the list's content, less half of
/// what the list shows beside the row; never past either end.
export function centredTop(row: { top: number; height: number }, list: { scrollHeight: number; clientHeight: number }): number {
  const top = row.top - (list.clientHeight - row.height) / 2;
  return Math.max(0, Math.min(top, list.scrollHeight - list.clientHeight));
}

/// Whether a row is wholly in sight in a list: its top and bottom, from the top of the list's content, within what the
/// list shows.
export function inSight(row: { top: number; height: number }, list: { scrollTop: number; clientHeight: number }): boolean {
  return row.top >= list.scrollTop && row.top + row.height <= list.scrollTop + list.clientHeight;
}
