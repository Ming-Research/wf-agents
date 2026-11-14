// What the owner does to a work from its page: pause it, let it on, have a line begin a new session, and cancel it.
// Each sends an `Act`, and its control is disabled until the answer comes, so that nothing is sent twice; where the
// interface refuses one, its reason is shown where the act was.

import { useRef, useState } from 'react';

import { Button } from '../parts';
import type { Act, Step, Summary } from '../works-api';
import { CANCEL_DOES, CANCEL_WHY, hasEnded, letGoFailed } from './words';

/// Sends an act, and gives why it was refused, or none where it was taken.
export type Send = (what: Act) => Promise<string | null>;

/// An act on its way: whether one is, and why the last was refused. `go` sends nothing while one is on its way, and
/// says whether what it sent was taken.
export function useAct(send: Send) {
  const [pending, setPending] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const sending = useRef(false);
  const go = async (what: Act): Promise<boolean> => {
    if (sending.current) return false;
    sending.current = true;
    setPending(true);
    const why = await send(what);
    sending.current = false;
    setPending(false);
    setRefused(why);
    return why === null;
  };
  return { pending, refused, go };
}

function Refused({ why }: { why: string | null }) {
  return why === null ? null : (
    <span className="failed small" role="alert">
      Refused: {why}
    </span>
  );
}

/// Letting go of what an ended work held failed, and why, with what tries it again: letting the work on. The Works
/// view and the console's window both show it.
export function LetGoAgain({ why, send }: { why: string; send: Send }) {
  const { pending, refused, go } = useAct(send);
  return (
    <span className="acts">
      <span className="failed small">Letting go of what it held failed: {why}</span>
      <Button
        disabled={pending}
        onClick={() => void go({ kind: 'resume' })}
        title="Let go of the work's worktree and pull request again"
      >
        Try again
      </Button>
      <Refused why={refused} />
    </span>
  );
}

/// Pause, let on, a new session for a line, and cancel; cancel asks first, and for why, which is kept with it, and a
/// new session asks for which line. A work that ended has none of these, but where letting go of what it held failed, that
/// it is tried again.
export function ActButtons({
  summary,
  steps,
  lines,
  send,
}: {
  summary: Summary;
  steps: Step[];
  lines: string[];
  send: Send;
}) {
  const [asking, setAsking] = useState(false);
  const [choosing, setChoosing] = useState(false);
  const [why, setWhy] = useState('');
  const { pending, refused, go } = useAct(send);
  const failed = letGoFailed(summary, steps);
  if (failed !== null) return <LetGoAgain why={failed} send={send} />;
  if (hasEnded(summary)) return null;
  const act = (what: Act) => {
    void go(what).then(() => {
      setAsking(false);
      setChoosing(false);
    });
  };
  return (
    <span className="acts">
      {summary.paused ? (
        <Button disabled={pending} onClick={() => act({ kind: 'resume' })} title="Let rounds of the work begin again">
          Start
        </Button>
      ) : (
        <Button
          disabled={pending}
          onClick={() => act({ kind: 'pause' })}
          title="Let no round of the work begin; a round under way goes on"
        >
          Pause
        </Button>
      )}
      {lines.length > 0 &&
        (choosing ? (
          <span className="confirm" role="group" aria-label="A new session for which line?">
            <span className="small">A new session at the next round of</span>
            {lines.map((line) => (
              <Button key={line} disabled={pending} onClick={() => act({ kind: 'newSession', line })}>
                {line}
              </Button>
            ))}
            <Button disabled={pending} onClick={() => setChoosing(false)}>
              No
            </Button>
          </span>
        ) : (
          <Button
            disabled={pending}
            onClick={() => setChoosing(true)}
            title="Have a line of the work begin a new session at its next round"
          >
            New session
          </Button>
        ))}
      {asking ? (
        <span className="confirm" role="group" aria-label="Cancel the work?">
          <span className="small">Cancel the work? {CANCEL_DOES}</span>
          <input
            className="field"
            aria-label="Why it is cancelled"
            placeholder={CANCEL_WHY}
            value={why}
            onChange={(e) => setWhy(e.target.value)}
          />
          <Button disabled={pending || why.trim() === ''} onClick={() => act({ kind: 'cancel', why: why.trim() })}>
            Yes, cancel it
          </Button>
          <Button disabled={pending} onClick={() => setAsking(false)}>
            No
          </Button>
        </span>
      ) : (
        <Button disabled={pending} onClick={() => setAsking(true)}>
          Cancel
        </Button>
      )}
      <Refused why={refused} />
    </span>
  );
}
