// The line at the head of the works page that says how the copies of the database stand. The owner needs it to know
// whether the records survive the loss of the machine's disk, and a failure of keeping them must not wait in a log.
// It is plain when all is well (the newest copy is recent and kept off the machine), and says in words, in the colour of
// what it means, every other state: a failure red, the rest orange. The engine works the state out from its records and
// its clock (`crates/backup`), so the line reads it and works out none of its own.

import { useCallback } from 'react';

import type { Copies } from '../works-api';
import { useChanges, useReading, useSource } from './source';
import { duration, fullTime } from './words';

/// The state in words for the owner, as the engine gives it at its time `now`.
export function copiesWords(copies: Copies): string {
  const ago = (time: number | null) => (time === null ? 'never' : `${duration(copies.now - time)} ago`);
  const { state } = copies;
  switch (state.kind) {
    case 'ok':
      return `The newest copy of the database was taken ${ago(copies.lastTaken)}, and is kept off this machine`;
    case 'localOnly':
      return 'Copies of the database stay on this machine, on the disk they guard against losing: no place off it is set';
    case 'pending':
      return `${state.n} ${state.n === 1 ? 'copy' : 'copies'} of the database not yet kept off this machine`;
    case 'stale':
      return state.since === null
        ? 'No copy of the database has been taken'
        : `The newest copy of the database was taken ${ago(state.since)}`;
    case 'clock':
      return "A copy of the database is dated in the future: the engine's clock is wrong";
    case 'failed':
      return `Keeping the database's copies failed at ${state.step}: ${state.why}`;
  }
}

/// The tone of a state: none for all is well, red for a failure, orange for what the owner is to look at.
export function copiesTone(copies: Copies): 'quiet' | 'warn' | 'failed' {
  switch (copies.state.kind) {
    case 'ok':
      return 'quiet';
    case 'failed':
      return 'failed';
    default:
      return 'warn';
  }
}

export function CopiesLine() {
  const source = useSource();
  const load = useCallback(() => source.loadCopies(), [source]);
  const { value: copies, failed, read } = useReading(load);
  // The state changes when a step of keeping the copies ends, which the engine says by a change that holds no record,
  // and the line reads it again then, and whenever the stream opens again, when a change may have been missed. The
  // page asks on no period of its own.
  useChanges((changed) => {
    if (changed.copies) read();
  }, read);

  if (!copies) {
    return failed ? (
      <span className="copies failed" role="alert">
        The copies of the database could not be read: {failed}
      </span>
    ) : null;
  }
  const words = copiesWords(copies);
  const tone = copiesTone(copies);
  return (
    <span
      className={`copies ${tone}`}
      data-state={copies.state.kind}
      role={tone === 'quiet' ? undefined : 'alert'}
      title={failed ? `${words}. It could not be read again: ${failed}` : `${words}. As of ${fullTime(copies.now)}.`}
    >
      {words}
    </span>
  );
}
