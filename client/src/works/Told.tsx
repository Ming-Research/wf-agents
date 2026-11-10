// What a round was told and what came back of it, as its carrier kept them: two parts of a round in the panel of a
// run, each read when the owner first opens it. The message is shown as it was written; of what came back, the
// agent's closing words, its structured return (opened by itself), and how many turns it took, how long and by which
// models. A round tried more than once shows each try in order; one its carrier kept nothing of says so.

import type { ReactNode } from 'react';

import type { Returned, ToldPage, Try } from '../works-api';
import { Content, Opener, Unfold } from './marks';
import { duration } from './words';

/// How many turns a try took, how long, and by which models: `3 turns, 42s, a-model`; what the result does not give is
/// left out.
export function returnedNumbers(r: Returned & { kind: 'read' }): string {
  return [
    ...(r.turns !== null ? [`${r.turns} turn${r.turns === 1 ? '' : 's'}`] : []),
    ...(r.durationMs !== null ? [duration(r.durationMs)] : []),
    ...(r.models.length > 0 ? [r.models.join(', ')] : []),
  ].join(', ');
}

/// Each try, labelled where there are more than one; or that nothing was kept.
function Tries({ told, part }: { told: ToldPage; part: (t: Try, last: boolean) => ReactNode }) {
  const { tries } = told;
  if (tries.length === 0) return <p className="muted small">Nothing was kept of this round</p>;
  if (tries.length === 1) return part(tries[0], true);
  return tries.map((t, i) => (
    <div key={i}>
      <p className="label">try {i + 1}</p>
      {part(t, i === tries.length - 1)}
    </div>
  ));
}

/// What came back of a try.
function Back({ returned, underWay }: { returned: Returned | null; underWay: boolean }) {
  if (returned === null) {
    return <p className="muted small">{underWay ? 'Nothing yet: it is under way' : 'Nothing came back'}</p>;
  }
  if (returned.kind === 'unreadable') return <p className="small failed">The result could not be read: {returned.why}</p>;
  const numbers = returnedNumbers(returned);
  const structured = returned.structured;
  return (
    <>
      {returned.text === null ? <p className="muted small">It said no closing words</p> : <Content text={returned.text} />}
      {structured === null ? (
        <p className="muted small">It gave no structured return</p>
      ) : (
        <Opener label="structured return" load={async () => structured} />
      )}
      {numbers !== '' && <p className="muted small">{numbers}</p>}
    </>
  );
}

/// What the round was told, and what it returned, each opened by itself; `underWay` while the round is.
export function RoundTold({ load, underWay }: { load: () => Promise<ToldPage>; underWay: boolean }) {
  return (
    <>
      <Unfold
        label="what it was told"
        load={load}
        show={(told) => <Tries told={told} part={(t) => <Content text={t.message} />} />}
      />
      <Unfold
        label="what it returned"
        load={load}
        show={(told) => (
          <Tries told={told} part={(t, last) => <Back returned={t.returned} underWay={underWay && last} />} />
        )}
      />
    </>
  );
}
