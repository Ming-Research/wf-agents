// A change of files as the item window draws a passage's ```diff block (decision
// a-diff-is-drawn-as-the-text-it-changes): the block read as a unified diff, as git and the engine write one, and each
// part of a file it changes drawn as the text reads after the change, in the look of code, with each change in its
// place: the words removed struck through in red, then the words added in green; each line it draws with its number
// in the file after the change at its left, where the diff gives the numbers and the line is in that file. The words of the two sides are
// compared, and blanks are not: a passage wrapped again changes nothing. Two changes fewer than `CLOSE` words apart are
// one change, so that no change is read word by word. The lines of the diff that say what git keeps of a file (its
// modes, its blobs, the numbers of its lines) are not drawn, and a file's path is drawn only where the block changes
// several files; a block that is no unified diff is drawn as the code it is.

import { useMemo } from 'react';

/// A line of a part of a file a diff changes: kept, removed or added, and its text without its sign.
type Line = { sign: ' ' | '-' | '+'; text: string };

/// A part of a file a diff changes: the number its lines begin at in the file after the change, none where the diff
/// gives no numbers; and its lines.
type Hunk = { from: number | null; lines: Line[] };

/// What a diff says of one file: its path, where it says one; what it says of the file beside its lines, such as that
/// it is binary; and the parts of it that changed.
type FileChange = { path: string | null; notes: string[]; hunks: Hunk[] };

/// A piece of what a part of a file is drawn as: text as it reads after the change, words removed, or words added.
type Piece = { kind: 'same' | 'removed' | 'added'; text: string };

/// Whether a character is the sign a line of a part of a file begins with.
const isSign = (c: string): c is Line['sign'] => c === ' ' || c === '-' || c === '+';

/// A part of a file, as a unified diff begins it: the numbers of its lines on each side, which say where it begins and
/// how long it is.
const HUNK = /^@@ -\d+(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/// What git says of a file beside its change that the owner does not read: its modes, its blobs, how alike it is to
/// another. A rename's or a copy's path is said by the paths of its change.
const GIT = new RegExp(
  `^(?:${[
    'index',
    'old mode',
    'new mode',
    'deleted file mode',
    'new file mode',
    'similarity index',
    'dissimilarity index',
    'rename from',
    'rename to',
    'copy from',
    'copy to',
  ].join('|')}) `,
);

/// A file's path as a diff names it, without the side git puts before it; none for the side a file added or deleted
/// does not have.
const pathOf = (named: string): string | null =>
  named === '/dev/null' ? null : named.replace(/^[ab]\//, '').replace(/\t.*$/, '');

/// The files a unified diff changes; none where the text is no unified diff, as where it has no part of a file.
function readDiff(text: string): FileChange[] | null {
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  if (lines.at(-1) === '') lines.pop();
  const files: FileChange[] = [];
  let file: FileChange | null = null;
  const fresh = (): FileChange => {
    file = { path: null, notes: [], hunks: [] };
    files.push(file);
    return file;
  };
  const current = (): FileChange => file ?? fresh();
  // A diff with no line that begins a part of a file is read as one part, every line of it a line of that part.
  const whole = !lines.some((l) => HUNK.test(l));
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const git = /^diff --git a\/.* b\/(.*)$/.exec(line);
    if (git !== null) {
      fresh().path = git[1];
      i += 1;
      continue;
    }
    if (line.startsWith('--- ') && (lines[i + 1] ?? '').startsWith('+++ ')) {
      const on = current().hunks.length > 0 ? fresh() : current();
      on.path = pathOf(lines[i + 1].slice(4)) ?? pathOf(line.slice(4));
      i += 2;
      continue;
    }
    if (GIT.test(line)) {
      i += 1;
      continue;
    }
    const hunk = HUNK.exec(line);
    if (hunk !== null || whole) {
      // As many lines as the part's numbers say, or every line where the diff has no numbers; a line no sign begins
      // ends it, and a note that a file has no line break at its end belongs to no line.
      let removed = hunk === null ? Infinity : Number(hunk[1] ?? 1);
      let added = hunk === null ? Infinity : Number(hunk[3] ?? 1);
      const body: Line[] = [];
      for (i += hunk === null ? 0 : 1; i < lines.length && (removed > 0 || added > 0); i++) {
        const l = lines[i];
        if (l.startsWith('\\')) continue;
        const sign = l === '' ? ' ' : l[0];
        if (!isSign(sign)) break;
        body.push({ sign, text: l.slice(1) });
        if (sign !== '+') removed -= 1;
        if (sign !== '-') added -= 1;
      }
      while (i < lines.length && lines[i].startsWith('\\')) i += 1;
      if (whole && i < lines.length) return null;
      current().hunks.push({ from: hunk === null ? null : Number(hunk[2]), lines: body });
      continue;
    }
    current().notes.push(line);
    i += 1;
  }
  return files.some((f) => f.hunks.length > 0) ? files : null;
}

/// A word of a text, by where it starts and ends in it: a run of what is not blank, a character of the scripts that
/// write no blanks between words standing alone.
type Word = { text: string; start: number; end: number };

const WORD =
  /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]|[^\s\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]+/gu;

const wordsOf = (text: string): Word[] =>
  [...text.matchAll(WORD)].map((m) => ({ text: m[0], start: m.index, end: m.index + m[0].length }));

/// How few words two changes stand apart for them to be drawn as one: a judgment.
const CLOSE = 4;

/// How many pairs of words are compared at most, past what begins and ends both sides alike; past it the words between
/// are one change. The comparison takes memory as their number.
const COMPARED = 1_000_000;

/// The words of two sides that are kept, as pairs of their places, in their order: what begins and ends both alike,
/// and between them the most words both have in the same order.
function kept(a: string[], b: string[]): [number, number][] {
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head += 1;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) {
    tail += 1;
  }
  const pairs: [number, number][] = [];
  for (let k = 0; k < head; k++) pairs.push([k, k]);
  const n = a.length - head - tail;
  const m = b.length - head - tail;
  if (n > 0 && m > 0 && n * m <= COMPARED) {
    // How many words the rest of each side has in common from each pair of places, worked out from the ends.
    const longest = new Uint16Array((n + 1) * (m + 1));
    const at = (i: number, j: number) => i * (m + 1) + j;
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        longest[at(i, j)] =
          a[head + i] === b[head + j]
            ? longest[at(i + 1, j + 1)] + 1
            : Math.max(longest[at(i + 1, j)], longest[at(i, j + 1)]);
      }
    }
    for (let i = 0, j = 0; i < n && j < m; ) {
      if (a[head + i] === b[head + j]) {
        pairs.push([head + i, head + j]);
        i += 1;
        j += 1;
      } else if (longest[at(i + 1, j)] >= longest[at(i, j + 1)]) i += 1;
      else j += 1;
    }
  }
  for (let k = tail; k > 0; k--) pairs.push([a.length - k, b.length - k]);
  return pairs;
}

/// A run of words kept on both sides, by the places of its first word and of the word after its last.
type Run = { a: number; b: number; aEnd: number; bEnd: number };

/// What a part of a file is drawn as: its text after the change, with each change in its place.
function compare(before: string, after: string): Piece[] {
  const a = wordsOf(before);
  const b = wordsOf(after);
  if (before !== after && a.length === b.length && a.every((w, i) => w.text === b[i].text)) {
    // The two sides differ in their blanks alone, which drawing the text after the change would not show.
    return [
      { kind: 'removed', text: before },
      { kind: 'same', text: '\n' },
      { kind: 'added', text: after },
    ];
  }
  const runs: Run[] = [];
  for (const [i, j] of kept(
    a.map((w) => w.text),
    b.map((w) => w.text),
  )) {
    const last = runs.at(-1);
    if (last !== undefined && last.aEnd === i && last.bEnd === j) {
      last.aEnd += 1;
      last.bEnd += 1;
    } else runs.push({ a: i, b: j, aEnd: i + 1, bEnd: j + 1 });
  }
  // A run of fewer than `CLOSE` words between two changes is taken into them, so that they are drawn as one.
  const changedBefore = runs.length === 0 || runs[0].a > 0 || runs[0].b > 0;
  const final = runs.at(-1);
  const changedAfter = final === undefined || final.aEnd < a.length || final.bEnd < b.length;
  const apart = runs.filter(
    (r, k) =>
      r.aEnd - r.a >= CLOSE || (k === 0 && !changedBefore) || (k === runs.length - 1 && !changedAfter),
  );
  const out: Piece[] = [];
  const same = (text: string) => text !== '' && out.push({ kind: 'same', text });
  // Where the text after the change has been drawn to, and where each side's words have been drawn to.
  let drawn = 0;
  let aAt = 0;
  let bAt = 0;
  for (let k = 0; k <= apart.length; k++) {
    const next: Run | undefined = apart[k];
    const aNext = next?.a ?? a.length;
    const bNext = next?.b ?? b.length;
    const nextStart = next === undefined ? after.length : b[next.b].start;
    if (aNext > aAt || bNext > bAt) {
      const removed = aNext > aAt ? before.slice(a[aAt].start, a[aNext - 1].end) : '';
      if (bNext > bAt) {
        const added = after.slice(b[bAt].start, b[bNext - 1].end);
        same(after.slice(drawn, b[bAt].start));
        if (removed !== '') {
          out.push({ kind: 'removed', text: removed });
          same(removed.includes('\n') || added.includes('\n') ? '\n' : ' ');
        }
        out.push({ kind: 'added', text: added });
        drawn = b[bNext - 1].end;
      } else {
        // Words removed with none added in their place stand where the blanks between the words kept are, as they
        // stood before, with the blanks that followed them then.
        const between = after.slice(drawn, nextStart);
        same(between !== '' ? between : before.slice(aAt === 0 ? 0 : a[aAt - 1].end, a[aAt].start));
        out.push({ kind: 'removed', text: removed });
        if (next !== undefined) same(before.slice(a[aNext - 1].end, a[aNext].start));
        drawn = nextStart;
      }
    }
    if (next === undefined) break;
    same(after.slice(drawn, b[next.bEnd - 1].end));
    drawn = b[next.bEnd - 1].end;
    aAt = next.aEnd;
    bAt = next.bEnd;
  }
  same(after.slice(drawn));
  return out;
}

/// A line a part of a file is drawn as: the pieces on it, and its number in the file after the change, none where the
/// line is only in the file before it or the diff gives no numbers.
type Drawn = { number: number | null; pieces: Piece[] };

/// What a part of a file is drawn as, from its lines: the pieces compared, cut at each line break into the lines drawn,
/// each numbered as the file after the change counts it where any of it, or the break that ends it, is in that file.
function compareHunk(hunk: Hunk): Drawn[] {
  const pieces = compare(
    hunk.lines.flatMap((l) => (l.sign === '+' ? [] : [l.text])).join('\n'),
    hunk.lines.flatMap((l) => (l.sign === '-' ? [] : [l.text])).join('\n'),
  );
  const drawn: Drawn[] = [];
  let next = hunk.from;
  let on: Piece[] = [];
  let after = false;
  const end = (breakAfter: boolean) => {
    const inAfter = after || breakAfter;
    drawn.push({ number: inAfter && next !== null ? next : null, pieces: on });
    if (inAfter && next !== null) next += 1;
    on = [];
    after = false;
  };
  for (const p of pieces) {
    const parts = p.text.split('\n');
    parts.forEach((text, i) => {
      if (i > 0) end(p.kind !== 'removed');
      if (text !== '') {
        on.push({ kind: p.kind, text });
        if (p.kind !== 'removed') after = true;
      }
    });
  }
  if (on.length > 0 || drawn.length === 0) end(false);
  return drawn;
}

/// A ```diff block of a passage: each file it changes, named where it changes several, what it says of each beside
/// its lines, and each part of it that changed, a hairline between two.
export function Diff({ text }: { text: string }) {
  const files = useMemo(
    () => readDiff(text)?.map((f) => ({ path: f.path, notes: f.notes, hunks: f.hunks.map(compareHunk) })) ?? null,
    [text],
  );
  if (files === null) {
    return (
      <pre className="content">
        <code>{text}</code>
      </pre>
    );
  }
  const named = files.length > 1;
  return (
    <div className="content">
      {files.map((f, i) => (
        <div key={i} className={i > 0 ? 'cw-diff-apart' : undefined}>
          {named && f.path !== null && <div className="cw-diff-file">{f.path}</div>}
          {f.notes.map((note, k) => (
            <div key={k} className="cw-diff-file">
              {note}
            </div>
          ))}
          {f.hunks.map((lines, j) => (
            <div key={j} className={j > 0 ? 'cw-diff-apart' : undefined}>
              {lines.map((line, n) => (
                <div key={n} className="cw-diff-line">
                  <span className="cw-diff-number" aria-hidden="true">
                    {line.number ?? ''}
                  </span>
                  <span className="cw-diff-text">
                    {line.pieces.map((p, k) =>
                      p.kind === 'removed' ? (
                        <del key={k} className="cw-removed">
                          {p.text}
                        </del>
                      ) : p.kind === 'added' ? (
                        <ins key={k} className="cw-added">
                          {p.text}
                        </ins>
                      ) : (
                        p.text
                      ),
                    )}
                  </span>
                </div>
              ))}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
