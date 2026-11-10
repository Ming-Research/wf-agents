// A change of a text marked word by word, for the comparison of two definitions: a task is some three hundred words in
// several paragraphs, and the owner approves a change of one by reading it, so that a change of one word must show as
// that word, not as the whole text taken out and put in again.
//
// The texts are cut into tokens, each a word (a run of what is not white space), a run of white space within a line,
// or one line break, and the longest common subsequence of the two lists is what stays; the rest was taken out or put
// in. A line break is a token of its own, so that a change of one shows as a change.

/// A stretch of the marked text: what stayed, what was taken out, or what was put in.
export type Part = { kind: 'same' | 'del' | 'ins'; text: string };

/// Where more of the words than this changed, the two texts read better whole, one after the other, than marked.
export const MOST_CHANGED = 0.5;

/// The longest lists of tokens the table of the common subsequence is worked out for, as the product of their lengths:
/// two tasks of a few thousand words. Past it, what is between the common start and end is taken out and put in whole.
const MOST_CELLS = 4_000_000;

const tokens = (text: string): string[] => text.match(/\n|[^\S\n]+|\S+/g) ?? [];

const isWord = (token: string) => /\S/.test(token);

const isSpace = (token: string) => /^[^\S\n]+$/.test(token);

type Op = { kind: Part['kind']; token: string };

/// The tokens of `a` and `b`, which share no first and no last token, as what stays, what is taken out and what is
/// put in, by the longest common subsequence; where both are too long for the table, all taken out and put in.
function middle(a: string[], b: string[]): Op[] {
  const n = a.length;
  const m = b.length;
  if (n * m > MOST_CELLS) {
    return [...a.map((token): Op => ({ kind: 'del', token })), ...b.map((token): Op => ({ kind: 'ins', token }))];
  }
  // The length of the longest common subsequence of a[i..] and b[j..], at i * (m + 1) + j.
  const width = m + 1;
  const longest = new Uint32Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      longest[i * width + j] =
        a[i] === b[j]
          ? longest[(i + 1) * width + j + 1] + 1
          : Math.max(longest[(i + 1) * width + j], longest[i * width + j + 1]);
    }
  }
  const ops: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) {
      ops.push({ kind: 'same', token: a[i] });
      i += 1;
      j += 1;
    } else if (j >= m || (i < n && longest[(i + 1) * width + j] >= longest[i * width + j + 1])) {
      ops.push({ kind: 'del', token: a[i] });
      i += 1;
    } else {
      ops.push({ kind: 'ins', token: b[j] });
      j += 1;
    }
  }
  return ops;
}

/// The text after, with what changed from the text before marked: each stretch that stayed, and between them what was
/// taken out followed by what was put in.
export function diffWords(before: string, after: string): Part[] {
  const a = tokens(before);
  const b = tokens(after);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start += 1;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end += 1;
  const ops: Op[] = [
    ...a.slice(0, start).map((token): Op => ({ kind: 'same', token })),
    ...middle(a.slice(start, a.length - end), b.slice(start, b.length - end)),
    ...a.slice(a.length - end).map((token): Op => ({ kind: 'same', token })),
  ];
  // A space alone that stayed between two changes is taken into them: "one two three" to "one four five three" reads
  // as "two" put out and "four five" put in, not as two changes with a space kept between them.
  for (let k = 1; k < ops.length - 1; k += 1) {
    if (ops[k].kind === 'same' && isSpace(ops[k].token) && ops[k - 1].kind !== 'same' && ops[k + 1].kind !== 'same') {
      ops.splice(k, 1, { kind: 'del', token: ops[k].token }, { kind: 'ins', token: ops[k].token });
      k += 1;
    }
  }
  const parts: Part[] = [];
  let taken = '';
  let put = '';
  const flush = () => {
    if (taken) parts.push({ kind: 'del', text: taken });
    if (put) parts.push({ kind: 'ins', text: put });
    taken = '';
    put = '';
  };
  for (const op of ops) {
    if (op.kind === 'del') taken += op.token;
    else if (op.kind === 'ins') put += op.token;
    else {
      flush();
      const last = parts[parts.length - 1];
      if (last?.kind === 'same') last.text += op.token;
      else parts.push({ kind: 'same', text: op.token });
    }
  }
  flush();
  return parts;
}

/// The share of the words of both texts that were taken out or put in: none when the texts are the same, all when one
/// of them is empty.
export function changedShare(parts: Part[]): number {
  let changed = 0;
  let all = 0;
  for (const part of parts) {
    const words = tokens(part.text).filter(isWord).length;
    // A word that stayed is a word of both texts.
    all += part.kind === 'same' ? 2 * words : words;
    if (part.kind !== 'same') changed += words;
  }
  return all === 0 ? 0 : changed / all;
}

/// The change from one text to the other marked word by word, or none where more than `MOST_CHANGED` of the words
/// changed, and the two texts are better read whole.
export function markedChange(before: string, after: string): Part[] | null {
  const parts = diffWords(before, after);
  return changedShare(parts) > MOST_CHANGED ? null : parts;
}
