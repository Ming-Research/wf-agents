// Where a work's time went, as the item window draws it: one block for each stretch of a model or of the tools and one
// for each stretch no round ran between them, from the work's first round to its last, on a muted ground. Brought over
// from the page on `main` (`client/src/charts.tsx`, `TimeTrack` and `barSpans`), as it was.

import type { WorkPage } from '../works-api';

/// What a stretch of a work's time was: a model at work, the tools running, or no round running.
export type TrackKind = 'model' | 'tool' | 'gap';

/// One stretch of a track, from `left` for `width`, both in percent of the track.
export type TrackSpan = { key: string; kind: TrackKind; left: number; width: number };

const pct = (part: number, whole: number) => (whole > 0 ? (part / whole) * 100 : 0);

/// A work's time laid out from its first round's start to its last round's end, each block at least two pixels wide.
export function TimeTrack({ spans }: { spans: TrackSpan[] }) {
  return (
    <div className="us-track">
      {spans.map((s) => (
        <span
          key={s.key}
          className={`us-block us-${s.kind}`}
          style={{ left: `${s.left}%`, width: `max(${s.width}%, 2px)` }}
        />
      ))}
    </div>
  );
}

/// The stretches of a track from the bar the engine lays a work's time out in (`Summary.bar`): one character for each
/// cell of equal width, `m` where a model worked, `t` where the tools did, `.` where no one did. A run of the same kind
/// is one block; the stretches between the first block and the last where no one worked are blocks of no round, and
/// the ends before and after are left bare.
export function barSpans(bar: string): TrackSpan[] {
  const cells = [...bar];
  const first = cells.findIndex((c) => c !== '.');
  if (first < 0) return [];
  const last = cells.length - 1 - [...cells].reverse().findIndex((c) => c !== '.');
  const kindOf = (c: string): TrackKind | null => (c === 'm' ? 'model' : c === 't' ? 'tool' : c === '.' ? 'gap' : null);
  const spans: TrackSpan[] = [];
  for (let i = first; i <= last; ) {
    let j = i;
    while (j < last && cells[j + 1] === cells[i]) j++;
    const kind = kindOf(cells[i]);
    if (kind !== null) spans.push({ key: String(i), kind, left: pct(i, cells.length), width: pct(j - i + 1, cells.length) });
    i = j + 1;
  }
  return spans;
}

/// How many equal cells a work's bar is cut into, as the engine cuts it.
const BAR_CELLS = 96;

/// A work's bar worked out from its page, by the engine's own rule (`Summary.bar`), for an engine that does not give
/// it: a round of an agent's cell is a model's; a round of an engine cell, and a tool's call from when it was made to
/// when it was done, are the tools'; the one under way is counted to `now`.
export function barOf(page: WorkPage, now = Date.now()): string {
  const agent = new Set(page.flow.cells.filter((c) => c.act === null).map((c) => c.id));
  const stretches: [number, number, boolean][] = [];
  for (const s of page.steps) {
    if (s.kind !== 'run') continue;
    for (const r of s.rounds) stretches.push([r.began, Math.max(r.began, r.ended ?? now), agent.has(s.cell)]);
    for (const c of s.calls)
      if (c.what.kind === 'tool') stretches.push([c.made, Math.max(c.made, c.done ?? now), false]);
  }
  const rounds = page.steps.flatMap((s) => (s.kind === 'run' ? s.rounds : []));
  if (rounds.length === 0) return '';
  const from = Math.min(...rounds.map((r) => r.began));
  const until = Math.max(...rounds.map((r) => Math.max(r.began, r.ended ?? now)));
  const span = until - from;
  if (span <= 0) return '';
  const cells = Array.from({ length: BAR_CELLS }, () => [0, 0]);
  for (const [a, b, model] of stretches) {
    const [start, stop] = [Math.min(until, Math.max(from, a)), Math.min(until, Math.max(from, b))];
    if (stop <= start) continue;
    const [s, e] = [(start - from) * BAR_CELLS, (stop - from) * BAR_CELLS];
    const first = Math.min(BAR_CELLS - 1, Math.floor(s / span));
    const last = Math.min(BAR_CELLS - 1, Math.floor(Math.max(0, e - BAR_CELLS) / span));
    for (let i = first; i <= last; i++) {
      const lo = span * i;
      cells[i][model ? 0 : 1] += Math.max(0, Math.min(e, lo + span) - Math.max(s, lo));
    }
  }
  return cells.map(([m, t]) => ((m + t) * 2 < span ? '.' : m >= t ? 'm' : 't')).join('');
}
