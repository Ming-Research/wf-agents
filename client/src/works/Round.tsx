// What a round's part of a run's panel shows of the round beside what it was given and what came back, each by its key
// as the flow's files and the work's records write it, and what the page works out from them said plainly: what it
// ran on and in which session of its line; what it took; the context it ended with; what was filled into its session,
// which is what was new to the round; while it is under way and its panel is open, what its agent is doing, read again
// every few seconds; and the system prompt of its session, which each of its rounds is given.

import { type ReactNode, useEffect, useState } from 'react';

import { Attr, ListRow } from '../parts';
import type { Doing, Fill, Round, RoundTry, Spent } from '../works-api';
import { Content, Unfold } from './marks';
import { type Key, count, spentKeys } from './words';

/// How often what a round's agent is doing is read again while the round is under way: the stream it is read from grows
/// with nothing the engine says has happened, so the page asks.
export const DOING_MS = 3000;

/// Keys and their values in a row, wrapping where the row is narrow.
export function Keys({ keys }: { keys: [string, ReactNode][] }) {
  return (
    <span className="keys">
      {keys.map(([key, value]) => (
        <Attr key={key} name={key} value={value} />
      ))}
    </span>
  );
}

/// The session of its line the round runs in, as the records count it, and what its last try ran on.
export function RanKeys({ round }: { round: Round }) {
  const session = round.session;
  const ran = round.tries[round.tries.length - 1]?.ran;
  const keys: Key[] = [
    ...(session ? ([['line', session.line], ['session', String(session.number)]] as Key[]) : []),
    ...(ran ? ([['level', ran.level], ['model', ran.model]] as Key[]) : []),
    ...(ran?.effort ? ([['effort', ran.effort]] as Key[]) : []),
  ];
  return <Keys keys={keys} />;
}

/// What the round took.
export function TookKeys({ spent }: { spent: Spent }) {
  return <Keys keys={spentKeys(spent)} />;
}

/// The context the round ended with, and how often its session was compacted during it.
export function ContextKeys({ context, compactions }: { context: number; compactions: number }) {
  return (
    <Keys
      keys={[
        ['context_tokens', count(context)],
        ['compactions', String(compactions)],
      ]}
    />
  );
}

/// A version of a block, named by its place: `plan@12`.
function Version({ block, at }: { block: string; at: number }) {
  return (
    <>
      {block}
      <span className="muted">@{at}</span>
    </>
  );
}

/// One thing filled into the session, by the part of the message it fills as the flow's files name it.
function FillRow({ fill }: { fill: Fill }) {
  switch (fill.kind) {
    case 'part':
      return (
        <ListRow>
          <span className="mono">{fill.part}</span>
        </ListRow>
      );
    case 'whole':
      return (
        <ListRow>
          <Attr name="context" value={<Version block={fill.block} at={fill.version} />} />
        </ListRow>
      );
    case 'change':
      return (
        <ListRow>
          <Attr name="context" value={<Version block={fill.block} at={fill.version} />} />
          <Attr name="since" value={`@${fill.since}`} />
        </ListRow>
      );
    case 'results':
      return (
        <ListRow>
          <Attr name="results" value={`[${fill.calls.join(', ')}]`} />
        </ListRow>
      );
    case 'sentBack':
      return (
        <ListRow>
          <Attr name="sent_back" value={`round ${fill.round}`} />
        </ListRow>
      );
  }
}

/// What was filled into the session of each try, each time it was: what the session lacked of what the cell is given.
/// The try is named only where there is more than one filling to tell apart.
export function Filled({ tries }: { tries: RoundTry[] }) {
  const fillings = tries.flatMap((t) => t.filled.map((fills, i) => ({ attempt: t.attempt, again: i > 0, fills })));
  const named = fillings.length > 1;
  return (
    <>
      {fillings.map((f) => (
        <div key={`${f.attempt} ${f.again ? 'again' : ''}`} className="filling">
          {named && (
            <span className="muted small">
              try {f.attempt}
              {f.again ? ', its session begun again' : ''}
            </span>
          )}
          {f.fills.length === 0 ? (
            <span className="muted mono">[]</span>
          ) : (
            <ul>
              {f.fills.map((fill, i) => (
                <FillRow key={i} fill={fill} />
              ))}
            </ul>
          )}
        </div>
      ))}
    </>
  );
}

/// One thing the agent was seen doing: a tool by its name and what its input names, what it said, or the compaction of
/// its session, as the stream writes it.
function DoingRow({ doing }: { doing: Doing }) {
  switch (doing.kind) {
    case 'tool':
      return (
        <ListRow>
          <span className="mono">{doing.name}</span>
          {doing.on !== null && <span className="mono muted call-input">{doing.on}</span>}
        </ListRow>
      );
    case 'said':
      return (
        <ListRow>
          <span>{doing.text}</span>
        </ListRow>
      );
    case 'compacting':
      return (
        <ListRow>
          <span className="mono">compact_boundary</span>
        </ListRow>
      );
  }
}

/// What the agent of a round under way is doing, the latest last: read at once and again every few seconds while it is
/// shown (`shown`) and the page is in sight; a read that fails keeps what was shown and says why.
export function DoingNow({ load, shown }: { load: () => Promise<Doing[]>; shown: boolean }) {
  const [seen, setSeen] = useState<{ doing: Doing[] | null; failed: string | null }>({ doing: null, failed: null });
  useEffect(() => {
    if (!shown) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const again = () => {
      if (!stopped) timer = setTimeout(look, DOING_MS);
    };
    const look = () => {
      if (typeof document !== 'undefined' && document.hidden) {
        again();
        return;
      }
      load().then(
        (doing) => {
          if (!stopped) setSeen({ doing, failed: null });
          again();
        },
        (e: unknown) => {
          if (!stopped) setSeen((s) => ({ doing: s.doing, failed: e instanceof Error ? e.message : String(e) }));
          again();
        },
      );
    };
    look();
    return () => {
      stopped = true;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [load, shown]);
  const { doing, failed } = seen;
  return (
    <>
      {doing === null && failed === null && <span className="muted small">Reading</span>}
      {doing !== null &&
        (doing.length === 0 ? (
          <span className="muted mono">[]</span>
        ) : (
          <ul className="doing">
            {doing.map((d, i) => (
              <DoingRow key={i} doing={d} />
            ))}
          </ul>
        ))}
      {failed !== null && <p className="small failed">It could not be read: {failed}</p>}
    </>
  );
}

/// The line's system prompt the round's session began with, which each of its rounds is given, opened by the owner.
export function SystemPrompt({ load }: { load: () => Promise<string> }) {
  return <Unfold label="system prompt" load={load} show={(text) => <Content text={text} />} />;
}
