// What a round was told and what came back of it, as its carrier kept them: two parts of a round in the panel of a
// run, each read when the owner first opens it, and read again once the round under way when it was read has ended.
// The message is shown as it was written; of what came back, the agent's closing words where they are not the
// structured return itself, its structured return, and, by the keys the program printed them under, how many turns it
// took, how long, by which models and how many tokens it wrote to the prompt cache; or that the try failed and why. A
// round tried more than once shows each try in order; one its carrier kept nothing of says so; what was cut or left
// out of a long round is said.

import type { ReactNode } from 'react';

import { Attr } from '../parts';
import type { ToldPage, Try, TryBack } from '../works-api';
import { Content, Opener, Unfold } from './marks';
import { count } from './words';

/// A length in bytes, in words: `512 bytes`, `3.2 KiB`, `1.5 MiB`.
export function bytesWords(n: number): string {
  if (n < 1024) return `${n} bytes`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KiB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MiB`;
}

/// How many turns a try took, how long, by which models and how many tokens it wrote to the prompt cache, each by the
/// key the program printed it under, with its value: `num_turns 3`; what the result does not give is left out.
export function returnedKeys(r: TryBack & { kind: 'returned' }): [string, string][] {
  const cacheWrites = r.cacheCreationInputTokens ?? null;
  return [
    ...(r.turns !== null ? [['num_turns', count(r.turns)] as [string, string]] : []),
    ...(r.durationMs !== null ? [['duration_ms', count(r.durationMs)] as [string, string]] : []),
    ...(r.models.length > 0 ? [['modelUsage', r.models.join(', ')] as [string, string]] : []),
    ...(cacheWrites !== null ? [['cache_creation_input_tokens', count(cacheWrites)] as [string, string]] : []),
  ];
}

/// Each try, labelled where there are more than one, and the tries left out; or that nothing is there.
function Tries({ told, underWay, part }: {
  told: ToldPage;
  underWay: boolean;
  part: (t: Try, last: boolean) => ReactNode;
}) {
  const { tries, leftOut } = told;
  if (tries.length === 0) {
    return <p className="muted small">{underWay ? 'Nothing yet: it is under way' : 'Nothing was kept of this round'}</p>;
  }
  const last = (i: number) => i === tries.length - 1 && leftOut === 0;
  return (
    <>
      {tries.length === 1 && leftOut === 0
        ? part(tries[0], true)
        : tries.map((t, i) => (
            <div key={i}>
              <p className="label">try {i + 1}</p>
              {part(t, last(i))}
            </div>
          ))}
      {leftOut > 0 && (
        <p className="muted small">
          {leftOut} later {leftOut === 1 ? 'try is' : 'tries are'} left out: the tries before {leftOut === 1 ? 'it' : 'them'}{' '}
          are as much as is read of a round
        </p>
      )}
    </>
  );
}

/// What a try was told, and where it was cut, its whole length.
function Message({ t }: { t: Try }) {
  return (
    <>
      <Content text={t.message} />
      {t.cut !== null && (
        <p className="muted small">Cut: only its start is shown; the whole message is {bytesWords(t.cut)}</p>
      )}
    </>
  );
}

/// What came back of a try.
function Back({ back, underWay }: { back: TryBack; underWay: boolean }) {
  if (back.kind === 'nothing') {
    return <p className="muted small">{underWay ? 'Nothing yet: it is under way' : 'Nothing came back'}</p>;
  }
  if (back.kind === 'failed') return <p className="small failed">The try failed: {back.why}</p>;
  if (back.kind === 'unreadable') return <p className="small failed">The result could not be read: {back.why}</p>;
  const keys = returnedKeys(back);
  const { text, structured } = back;
  return (
    <>
      {text !== null && <Content text={text} />}
      {structured !== null &&
        (text === null ? <Content text={structured} /> : <Opener label="structured return" load={async () => structured} />)}
      {text === null && structured === null && <p className="muted small">It said no closing words</p>}
      {keys.length > 0 && (
        <p className="keys small">
          {keys.map(([key, value]) => (
            <Attr key={key} name={key} value={value} />
          ))}
        </p>
      )}
    </>
  );
}

/// What the round was told, and what it returned, each opened by itself; `ended` is when the round ended, none while
/// it is under way.
export function RoundTold({ load, ended }: { load: () => Promise<ToldPage>; ended: number | null }) {
  const underWay = ended === null;
  return (
    <>
      <Unfold
        label="what it was told"
        load={load}
        of={ended}
        show={(told) => <Tries told={told} underWay={underWay} part={(t) => <Message t={t} />} />}
      />
      <Unfold
        label="what it returned"
        load={load}
        of={ended}
        show={(told) => (
          <Tries told={told} underWay={underWay} part={(t, last) => <Back back={t.back} underWay={underWay && last} />} />
        )}
      />
    </>
  );
}
