// What the owner does to a work from its page that the item's window does not hold: have a line begin a new session.
// It sends an `Act`, and its control is disabled until the answer comes, so that nothing is sent twice; where the
// interface refuses one, its reason is shown where the act was.

import { useRef, useState } from 'react';

import { Button } from '../parts';
import type { Act } from '../works-api';

/// Sends an act, and gives why it was refused, or none where it was taken.
export type Send = (what: Act) => Promise<string | null>;

/// An act on its way: whether one is, and why the last was refused. `go` sends nothing while one is on its way, and
/// says whether what it sent was taken.
function useAct(send: Send) {
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

/// A new session for a line of the work, at its next round, asking first which line.
export function NewSession({ lines, send }: { lines: string[]; send: Send }) {
  const [choosing, setChoosing] = useState(false);
  const { pending, refused, go } = useAct(send);
  if (lines.length === 0) return null;
  const act = (what: Act) => {
    void go(what).then(() => setChoosing(false));
  };
  return (
    <span className="acts">
      {choosing ? (
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
      )}
      {refused !== null && (
        <span className="failed small" role="alert">
          Refused: {refused}
        </span>
      )}
    </span>
  );
}
