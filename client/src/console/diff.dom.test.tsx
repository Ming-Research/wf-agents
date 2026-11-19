// @vitest-environment happy-dom
// A diff drawn as the text it changes (decision a-diff-is-drawn-as-the-text-it-changes): each change in its place, the
// words removed struck through and then the words added, two changes close together drawn as one, blanks compared only
// where nothing else differs; its lines numbered as the file after the change counts them; what git keeps of a file
// left out, and a file's path drawn only where the diff changes several; and the words coloured by their kind, each
// side of a part of a file read whole.
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Diff } from './diff';

afterEach(() => {
  document.body.innerHTML = '';
});

function draw(text: string, path: string | null = null, folded = false): HTMLElement {
  const container = document.body.appendChild(document.createElement('div'));
  const root = createRoot(container);
  act(() => root.render(<Diff text={text} path={path} folded={folded} />));
  return container;
}

const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((e) => e.textContent ?? '');

/// Each line drawn: its number, and its pieces, kept as they are, removed as `-[…]`, added as `+[…]`.
const lines = (root: ParentNode) =>
  [...root.querySelectorAll('.cw-diff-line')].map((line) => {
    const number = line.querySelector('.cw-diff-number')?.textContent ?? '';
    const text = [...(line.querySelector('.cw-diff-text')?.children ?? [])]
      .map((p) => (p.tagName === 'DEL' ? `-[${p.textContent}]` : p.tagName === 'INS' ? `+[${p.textContent}]` : p.textContent))
      .join('');
    return `${number}|${text}`;
  });

const ONE_FILE = [
  'diff --git a/src/time.rs b/src/time.rs',
  'index 1a2b3c4..5d6e7f8 100644',
  '--- a/src/time.rs',
  '+++ b/src/time.rs',
  '@@ -10,3 +10,3 @@ mod time',
  ' fn keep() {}',
  '-let ms = 1000;',
  '+let ms = 2000;',
  ' fn end() {}',
].join('\n');

describe("a diff in the item's window", () => {
  it('draws the text after the change, each change in its place, the removed words struck in red and the added in green', () => {
    const el = draw(ONE_FILE);
    expect(lines(el)).toEqual(['10|fn keep() {}', '11|let ms = -[1000;] +[2000;]', '12|fn end() {}']);
    expect(el.querySelector('del')?.className).toBe('cw-removed');
    expect(el.querySelector('ins')?.className).toBe('cw-added');
    // What git keeps of a file and the diff's own signs and numbers are not drawn, nor the path of its one file.
    expect(el.textContent).not.toContain('index');
    expect(el.textContent).not.toContain('@@');
    expect(el.textContent).not.toContain('src/time.rs');
    expect(el.querySelector('.cw-diff-file')).toBeNull();
  });

  it('colours the words of each piece by their kind in the language of the file, struck and added words too', () => {
    const el = draw(ONE_FILE);
    expect(texts(el, '.cw-diff-line .code-keyword')).toEqual(['fn', 'let', 'fn']);
    expect(texts(el, 'del .code-literal')).toEqual(['1000']);
    expect(texts(el, 'ins .code-literal')).toEqual(['2000']);
    // A file the page knows no language of is drawn in the text's colour.
    expect(draw(ONE_FILE.replaceAll('time.rs', 'time.unknown')).querySelector('[class^="code-"]')).toBeNull();
    // The path given with the diff names its language where the diff names none.
    const bare = '@@ -1 +1 @@\n-let a = 1;\n+let a = 2;';
    expect(texts(draw(bare, 'src/a.rs'), '.code-keyword')).toEqual(['let']);
  });

  it('reads each side of a part of a file whole, so that a comment over several lines is one on each of them', () => {
    const el = draw(['--- a/x.ts', '+++ b/x.ts', '@@ -1,2 +1,2 @@', ' /* one', '-two */ let a = 1;', '+two */ let b = 1;'].join('\n'));
    expect(lines(el)).toEqual(['1|/* one', '2|two */ let -[a] +[b] = 1;']);
    expect(texts(el, '.code-comment')).toEqual(['/* one', 'two */']);
    expect(texts(el, '.code-keyword')).toEqual(['let']);
  });

  it('draws two changes fewer than four words apart as one, and a passage wrapped again as changing only its words', () => {
    expect(lines(draw('-the cell runs once\n+the flow runs twice'))).toEqual(['|the -[cell runs once] +[flow runs twice]']);
    // Four words apart, each change stands in its place.
    expect(lines(draw('-a b c d e f\n+a x c d e f y'))).toEqual(['|a -[b] +[x] c d e f +[y]']);
    // The blanks are not compared: the passage's lines broken elsewhere change nothing but the word changed.
    expect(lines(draw('-alpha beta gamma delta\n-epsilon zeta\n+alpha beta gamma\n+delta epsilon eta'))).toEqual([
      '|alpha beta gamma',
      '|delta epsilon -[zeta] +[eta]',
    ]);
    // Where only the blanks differ, the two sides are drawn whole, the one removed and the one added.
    expect(lines(draw('-a  b\n+a b'))).toEqual(['|-[a  b]', '|+[a b]']);
  });

  it('numbers the lines as the file after the change counts them, and none that is only in the file before it', () => {
    const el = draw(['@@ -5,3 +5,2 @@', ' kept line', '-gone line here', '-and gone', ' last line'].join('\n'));
    expect(lines(el)).toEqual(['5|kept line', '|-[gone line here]', '|-[and gone]', '6|last line']);
    // Lines replaced whole: the break the page puts between the words removed and the words added is in neither file.
    expect(lines(draw('@@ -1,2 +1,2 @@\n-a\n-b\n+c\n+d'))).toEqual(['|-[a]', '|-[b]', '1|+[c]', '2|+[d]']);
  });

  it("names each file where the diff changes several, says what it says of one beside its lines, and draws what is no diff as code", () => {
    const two = [
      'diff --git a/a.md b/a.md',
      '--- a/a.md',
      '+++ b/a.md',
      '@@ -1 +1 @@',
      '-one',
      '+two',
      'diff --git a/logo.png b/logo.png',
      'Binary files a/logo.png and b/logo.png differ',
    ].join('\n');
    const el = draw(two);
    expect(texts(el, '.cw-diff-file')).toEqual(['a.md', 'logo.png', 'Binary files a/logo.png and b/logo.png differ']);
    const code = draw('hello\nworld');
    expect(code.querySelector('pre.content code')?.textContent).toBe('hello\nworld');
    expect(code.querySelector('.cw-diff-line')).toBeNull();
  });

  it('folds each file of a diff away in a row that opens it, as a file the work changed is', () => {
    const two = [
      'diff --git a/a.md b/a.md',
      '--- a/a.md',
      '+++ b/a.md',
      '@@ -1 +1 @@',
      '-one',
      '+two',
      'diff --git a/b.md b/b.md',
      '--- a/b.md',
      '+++ b/b.md',
      '@@ -1 +1 @@',
      '-three',
      '+four',
    ].join('\n');
    const el = draw(two, null, true);
    // One row for each file, named by its path where the diff holds several, and nothing drawn until one is opened.
    expect(texts(el, '.cw-file')).toEqual(['a.md', 'b.md']);
    expect(lines(el)).toEqual([]);
    act(() => (el.querySelectorAll('.cw-file')[1] as HTMLButtonElement).click());
    expect(lines(el)).toEqual(['1|-[three] +[four]']);
    // A diff of one file says only that it is the diff: the words it stands under name the file already.
    const one = ['diff --git a/a.md b/a.md', '--- a/a.md', '+++ b/a.md', '@@ -1 +1 @@', '-one', '+two'].join('\n');
    expect(texts(draw(one, null, true), '.cw-file')).toEqual(['The diff']);
    expect(texts(draw('@@ -1 +1 @@\n-one\n+two', null, true), '.cw-file')).toEqual(['The diff']);
  });
});
