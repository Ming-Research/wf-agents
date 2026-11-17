// How the copies of the database stand, in words, for the System view's part of what holds now. The owner needs it to
// know whether the records survive the loss of the machine's disk. It is plain when all is well (the newest copy is
// recent and kept off the machine), and says in words, in the colour of what it means, every other state: a failure
// red, the rest orange. The engine works the state out from its records and its clock (`crates/backup`), so the view
// reads it and works out none of its own; an engine that keeps no copies says so, and the view then shows none.

import type { Copies } from '../works-api';
import { duration } from '../works/words';

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
    case 'notKept':
      return 'This engine keeps no copies of the database: it runs a script';
  }
}

/// The tone of a state: none for all is well or for an engine that keeps none, red for a failure, orange for what the
/// owner is to look at.
export function copiesTone(copies: Copies): 'quiet' | 'warn' | 'failed' {
  switch (copies.state.kind) {
    case 'ok':
    case 'notKept':
      return 'quiet';
    case 'failed':
      return 'failed';
    default:
      return 'warn';
  }
}
