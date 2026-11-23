// What a time a cell ran shows in a run's panel beside what came back, each by its key as the flow's files and the
// work's records write it, and what the page works out from them said plainly: what it took; the context it ended
// with; how it missed its session's prompt cache, where it did; what its agent did, the latest two and the rest folded,
// followed as it comes while it is under way and read once after; and the system prompt of its session.

import { type ReactNode, useEffect, useRef, useState } from 'react';

import { REOPEN_MS } from '../changes';
import { Attr, ListRow } from '../parts';
import type { Doing, Done, Following, Missed, Spent } from '../works-api';
import { Content, Unfold } from './marks';
import { type Key, count, spentKeys } from './words';

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

/// What the round took, and, where it was worked out from its calls at their models' prices, that it was: `priced`.
export function TookKeys({ spent, estimated }: { spent: Spent; estimated: boolean }) {
  const keys: Key[] = estimated ? [...spentKeys(spent), ['priced', 'true']] : spentKeys(spent);
  return <Keys keys={keys} />;
}

/// What comes before the session's messages and differs from the call before, by name; `unknown` where the records do
/// not say why the round missed, and `none` where nothing differs, a fault of wf-agents.
function differsOf(missed: Missed): string {
  if (missed.differs.length > 0) return missed.differs.join(', ');
  return missed.unknown ? 'unknown' : 'none';
}

/// How the round missed its session's prompt cache: what its first call read from the cache, what the session held,
/// and why: what comes before the session's messages and differs from the call before.
export function MissedKeys({ missed }: { missed: Missed }) {
  return (
    <Keys
      keys={[
        ['first_read', count(missed.read)],
        ['held', count(missed.held)],
        ['differs', differsOf(missed)],
      ]}
    />
  );
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

/// How many of the latest things an agent did stand open; those before them are folded behind one line.
const OPEN_DONE = 2;

/// What the agent of a round did, in the order done, every try in order, the latest last: the latest two, and those
/// before them folded behind one line that says how many and opens them all. Of a round under way as the list is first
/// shown, each thing is added as the round's stream tells it, until the stream says all was told; a stream lost is
/// followed anew a few seconds later, passing over as many things as the list holds, which it tells again first. Of a
/// round that had ended, the whole is read once. Either is done only while the Step is `shown`, and what was shown
/// stands meanwhile; a read that fails says why.
export function DoneList({ underWay, follow, load, shown }: {
  underWay: boolean;
  follow: (following: Following) => () => void;
  load: () => Promise<Done[]>;
  shown: boolean;
}) {
  const [done, setDone] = useState<Done[] | null>(underWay ? [] : null);
  const [failed, setFailed] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  // Whether the round was under way as the list was first shown, which keeps it followed until its stream ends, though
  // the work's page says meanwhile that it ended; how many things the list holds; and whether all was told.
  const following = useRef(underWay);
  const held = useRef(0);
  const told = useRef(false);
  const latest = useRef({ follow, load });
  useEffect(() => {
    latest.current = { follow, load };
  });
  useEffect(() => {
    if (!shown || told.current) return;
    if (!following.current) {
      let alive = true;
      latest.current.load().then(
        (all) => {
          if (!alive) return;
          told.current = true;
          held.current = all.length;
          setDone(all);
          setFailed(null);
        },
        (e: unknown) => alive && setFailed(e instanceof Error ? e.message : String(e)),
      );
      return () => {
        alive = false;
      };
    }
    let stop: (() => void) | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const start = () => {
      timer = null;
      let pass = held.current;
      stop = latest.current.follow({
        done: (d) => {
          if (pass > 0) {
            pass -= 1;
            return;
          }
          held.current += 1;
          setDone((was) => [...(was ?? []), d]);
        },
        end: () => {
          stop = null;
          told.current = true;
        },
        failed: (why) => {
          stop = null;
          told.current = true;
          setFailed(why);
        },
        lost: () => {
          stop = null;
          timer = setTimeout(start, REOPEN_MS);
        },
      });
    };
    start();
    return () => {
      stop?.();
      if (timer !== null) clearTimeout(timer);
    };
  }, [shown]);
  const all = done ?? [];
  const before = Math.max(0, all.length - OPEN_DONE);
  const from = open ? 0 : before;
  return (
    <>
      {done === null && failed === null && <span className="muted small">Reading</span>}
      {done !== null && all.length === 0 && failed === null && <span className="muted mono">[]</span>}
      {before > 0 && (
        <button type="button" className="link small" aria-expanded={open} onClick={() => setOpen(!open)}>
          {open ? 'Hide' : 'Show'} {before} earlier
        </button>
      )}
      {all.length > 0 && (
        <ul className="doing">
          {all.slice(from).map((d, i) => (
            <DoingRow key={from + i} doing={d} />
          ))}
        </ul>
      )}
      {failed !== null && <p className="small failed">It could not be read: {failed}</p>}
    </>
  );
}

/// The line's system prompt the round's session began with, which each of its rounds is given, opened by the owner.
export function SystemPrompt({ load }: { load: () => Promise<string> }) {
  return <Unfold label="system prompt" load={load} show={(text) => <Content text={text} />} />;
}
