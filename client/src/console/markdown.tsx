// A passage of Markdown as the item window shows it, drawn with the page's own parts by a parser of its own (decision
// the-page-draws-markdown-itself): headings, each a small heading whatever its level (principle
// an-agents-heading-is-small); paragraphs; lists, every one numbered (the design of the owner's page, 14), each item
// holding blocks of its own; quotes; tables, in the page's table; rules; and blocks of code between fences, a change of
// files among them drawn as the text it changes (`diff.tsx`), a diagram drawn (`diagram.tsx`), and the words of one in
// a language the page knows coloured by their kind (`code.tsx`). In a line: `code`, **strong**, *stressed* and
// ~~struck~~ words, links, pictures, breaks, and a pointer to a question of the work, `{{name}}`, as the words of a
// cell's questions tell its agent to write one (decision a-question-stands-where-a-text-points-to-it). What stands in
// code is text, a pointer too. Nothing written becomes markup the page did not mean (principle agent-writing-is-data):
// a link leads only to the web or to a view of the page, a picture is drawn only from the web or the page's own
// server, and any other is shown as it was written.

import { Fragment, type ReactNode } from 'react';

import { CodeBlock, grammarNamed } from '../code';
import { Diagram, diagramKind } from '../diagram';
import { Diff } from './diff';

/// How a piece of a line is set: strong, stressed, struck, and the link it stands in, none where it stands in none.
type Marks = { strong: boolean; stressed: boolean; struck: boolean; href: string | null };

const PLAIN: Marks = { strong: false, stressed: false, struck: false, href: null };

/// A piece of a line: words, code, a picture, a break, or a pointer to the question of this name, as it was written.
type Inline = (
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'picture'; src: string; alt: string; title: string | null }
  | { kind: 'break' }
  | { kind: 'pointer'; name: string; written: string }
) &
  Marks;

/// How a column of a table is aligned, as its delimiter row says; none where it says nothing.
type Align = 'left' | 'center' | 'right' | null;

/// A block of a passage. A heading and a table's cells hold no question's card: a pointer there is only a link.
type Block =
  | { kind: 'paragraph'; pieces: Inline[] }
  | { kind: 'heading'; pieces: Inline[] }
  | { kind: 'code'; text: string; lang: string }
  | { kind: 'quote'; blocks: Block[] }
  | { kind: 'list'; start: number; items: Block[][] }
  | { kind: 'table'; align: Align[]; head: Inline[][]; rows: Inline[][][] }
  | { kind: 'rule' };

const FENCE = /^( {0,3})(`{3,}|~{3,})[ \t]*([^\s`]*)[^`]*$/;
const HEADING = /^ {0,3}#{1,6}(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const RULE = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
const ITEM = /^( *)([-*+]|\d{1,9}[.)])(?:[ \t]+(.*))?$/;
/// A quoted line: `>`, then one space if there is one, which is not part of what is quoted.
const QUOTE = /^ {0,3}> ?/;
/// A table's delimiter row: a run of dashes for each column, a colon at an end saying how it is aligned.
const DELIMITER = /^ {0,3}\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;

const blank = (l: string) => l.trim() === '';
const indentOf = (l: string) => l.length - l.trimStart().length;
const ordered = (marker: string) => /\d/.test(marker);

/// The cells of a row of a table, between its pipes; a pipe after a backslash is the cell's own.
function cellsOf(line: string): string[] {
  const row = line.trim().replace(/^\|/, '').replace(/(?<!\\)\|$/, '');
  return row.split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, '|'));
}

/// Whether a table begins at this line: a row of cells over a delimiter row of as many columns.
const tableAt = (lines: string[], i: number): boolean =>
  lines[i].includes('|') &&
  i + 1 < lines.length &&
  DELIMITER.test(lines[i + 1]) &&
  cellsOf(lines[i]).length === cellsOf(lines[i + 1]).length;

/// Whether a line begins a block of its own, and so ends a paragraph.
const begins = (lines: string[], i: number): boolean =>
  blank(lines[i]) ||
  FENCE.test(lines[i]) ||
  HEADING.test(lines[i]) ||
  RULE.test(lines[i]) ||
  QUOTE.test(lines[i]) ||
  ITEM.test(lines[i]) ||
  tableAt(lines, i);

/// The blocks of a passage.
function parse(text: string): Block[] {
  return blocksOf(text.replace(/\r\n?/g, '\n').split('\n'));
}

function blocksOf(lines: string[]): Block[] {
  const out: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (blank(line)) {
      i += 1;
      continue;
    }
    const fence = FENCE.exec(line);
    if (fence !== null) {
      // The lines up to a fence of the same sign at least as long, or to the end where none closes it, each with as
      // much of the opening fence's indent taken off as it has.
      const [, indent, open, lang] = fence;
      const close = new RegExp(`^ {0,3}${open[0]}{${open.length},}[ \\t]*$`);
      const body: string[] = [];
      for (i += 1; i < lines.length && !close.test(lines[i]); i++) {
        body.push(lines[i].slice(Math.min(indent.length, indentOf(lines[i]))));
      }
      i += 1;
      out.push({ kind: 'code', text: body.join('\n'), lang: lang.toLowerCase() });
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading !== null) {
      out.push({ kind: 'heading', pieces: inline(heading[1] ?? '', PLAIN) });
      i += 1;
      continue;
    }
    if (RULE.test(line)) {
      out.push({ kind: 'rule' });
      i += 1;
      continue;
    }
    if (QUOTE.test(line)) {
      // The lines that start with `>` are one quote, whose own blocks are what is under the marks.
      const quoted: string[] = [];
      while (i < lines.length && QUOTE.test(lines[i])) {
        quoted.push(lines[i].replace(QUOTE, ''));
        i += 1;
      }
      out.push({ kind: 'quote', blocks: blocksOf(quoted) });
      continue;
    }
    if (ITEM.test(line)) {
      i = list(lines, i, out);
      continue;
    }
    if (tableAt(lines, i)) {
      i = table(lines, i, out);
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && (para.length === 0 || !begins(lines, i))) {
      para.push(lines[i].trimStart());
      i += 1;
    }
    out.push({ kind: 'paragraph', pieces: inline(para.join('\n'), PLAIN) });
  }
  return out;
}

/// A list from line `start`: its items, each with the lines under it, which are blocks of their own (a list in an
/// item is a list in those blocks). Returns the line after the list.
function list(lines: string[], start: number, out: Block[]): number {
  const first = ITEM.exec(lines[start]);
  if (first === null) return start + 1;
  const indent = first[1].length;
  const isOrdered = ordered(first[2]);
  const items: string[][] = [];
  let item: string[] = [];
  let content = 0;
  let afterBlank = false;
  let i = start;
  while (i < lines.length) {
    const line = lines[i];
    if (blank(line)) {
      afterBlank = true;
      item.push('');
      i += 1;
      continue;
    }
    const m = ITEM.exec(line);
    const lead = indentOf(line);
    if (
      m !== null &&
      !RULE.test(line) &&
      m[1].length <= indent &&
      m[1].length >= indent - 1 &&
      ordered(m[2]) === isOrdered
    ) {
      item = [m[3] ?? ''];
      items.push(item);
      content = m[0].length - (m[3] ?? '').length;
      afterBlank = false;
      i += 1;
      continue;
    }
    if (lead > indent) {
      item.push(line.slice(Math.min(lead, content)));
      afterBlank = false;
      i += 1;
      continue;
    }
    // A line that only goes on with the item's text, not indented.
    if (!afterBlank && !begins(lines, i)) {
      item.push(line.trimStart());
      i += 1;
      continue;
    }
    break;
  }
  const n = isOrdered ? Number.parseInt(first[2], 10) : 1;
  out.push({ kind: 'list', start: n, items: items.map(blocksOf) });
  return i;
}

/// A table from line `start`, its head and its delimiter row there: its rows, up to a blank line or a line with no
/// pipe, each with as many cells as the head. Returns the line after the table.
function table(lines: string[], start: number, out: Block[]): number {
  const head = cellsOf(lines[start]);
  const align = cellsOf(lines[start + 1]).map((c): Align => {
    const left = c.startsWith(':');
    const right = c.endsWith(':');
    return left && right ? 'center' : right ? 'right' : left ? 'left' : null;
  });
  const rows: Inline[][][] = [];
  let i = start + 2;
  for (; i < lines.length && !blank(lines[i]) && lines[i].includes('|'); i++) {
    const cells = cellsOf(lines[i]);
    rows.push(head.map((_, c) => inline(cells[c] ?? '', PLAIN)));
  }
  out.push({ kind: 'table', align, head: head.map((c) => inline(c, PLAIN)), rows });
  return i;
}

/// A character a backslash may stand before, to be taken as itself.
const ESCAPED = /[!-/:-@[-`{-~]/;
/// A letter or a digit, of any script: an underscore between two is part of a name, never a mark.
const WORDLIKE = /[\p{L}\p{N}]/u;

/// Where the mark `sign` that ends a run of words begun at `from` stands: after something that is not a blank, a
/// single sign standing alone or before a double one; an underscore not before a letter or a digit. Code is skipped.
function closing(text: string, from: number, sign: string): number {
  for (let j = from; j < text.length; j++) {
    if (text[j] === '`') {
      let n = 1;
      while (text[j + n] === '`') n += 1;
      const end = codeEnd(text, j, n);
      j = end < 0 ? j + n - 1 : end + n - 1;
      continue;
    }
    if (text[j] === '\\') {
      j += 1;
      continue;
    }
    if (!text.startsWith(sign, j) || j === from || /\s/.test(text[j - 1])) continue;
    let run = 0;
    while (text[j + run] === sign[0]) run += 1;
    if (sign.length === 1 && run === 2) {
      j += 1;
      continue;
    }
    if (sign[0] === '_' && WORDLIKE.test(text[j + run] ?? '')) continue;
    return j;
  }
  return -1;
}

/// Where the code begun by `n` backticks at `at` ends: the next run of exactly `n` backticks; none where none closes
/// it.
function codeEnd(text: string, at: number, n: number): number {
  for (let j = at + n; j < text.length; j++) {
    if (text[j] !== '`') continue;
    let m = 1;
    while (text[j + m] === '`') m += 1;
    if (m === n) return j;
    j += m - 1;
  }
  return -1;
}

/// Where a link may lead: the web, a mail, or a view of this page by its address; anything else, such as a path in a
/// repository, is no link.
function linkTo(href: string): string | null {
  return /^(https?:\/\/|mailto:)/i.test(href) || href.startsWith('#/') ? href : null;
}

/// Where a picture may come from: the web, or the page's own server.
const pictureFrom = (src: string): boolean => /^https?:\/\//i.test(src) || /^\/(?!\/)/.test(src);

/// What a pattern finds where `i` stands in the text, none where it finds nothing there.
function found(pattern: RegExp, text: string, i: number): RegExpExecArray | null {
  pattern.lastIndex = i;
  return pattern.exec(text);
}

/// Two blanks or more at the end of a line, which break it there.
const HARD_BREAK = / {2,}\n/y;
const POINTER = /\{\{([^{}]+)\}\}/y;
const PICTURE = /!\[([^\]\n]*)\]/y;
const LINK = /\[((?:[^[\]\n]|\[[^[\]\n]*\])+)\]/y;
/// A link or a picture's target after its words, `(target "title")`, with the target between `<` and `>` where it has
/// blanks.
const TARGET = /\(\s*(?:<([^<>\n]*)>|([^\s()<>]+))(?:\s+"([^"\n]*)")?\s*\)/y;
const AUTOLINK = /<((?:https?:\/\/|mailto:)[^\s<>]+)>/iy;
/// The sign that begins a run of marked words.
const SIGN = /\*\*|__|~~|\*|_/y;

/// The pieces of the words of a line, set as `marks` say, which the marks written in it add to.
function inline(text: string, marks: Marks): Inline[] {
  const out: Inline[] = [];
  let buf = '';
  const flush = () => {
    if (buf !== '') out.push({ kind: 'text', text: buf, ...marks });
    buf = '';
  };
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '\\' && text[i + 1] === '\n') {
      flush();
      out.push({ kind: 'break', ...marks });
      i += 2;
      continue;
    }
    if (c === '\\' && ESCAPED.test(text[i + 1] ?? '')) {
      buf += text[i + 1];
      i += 2;
      continue;
    }
    const hard = found(HARD_BREAK, text, i);
    if (hard !== null) {
      flush();
      out.push({ kind: 'break', ...marks });
      i += hard[0].length;
      continue;
    }
    if (c === '`') {
      let n = 1;
      while (text[i + n] === '`') n += 1;
      const end = codeEnd(text, i, n);
      if (end < 0) {
        buf += '`'.repeat(n);
        i += n;
        continue;
      }
      flush();
      const code = text.slice(i + n, end).replace(/\n/g, ' ');
      out.push({ kind: 'code', text: /^ .*\S.* $/.test(code) ? code.slice(1, -1) : code, ...marks });
      i = end + n;
      continue;
    }
    const pointer = found(POINTER, text, i);
    if (pointer !== null) {
      flush();
      out.push({ kind: 'pointer', name: pointer[1], written: pointer[0], ...marks });
      i += pointer[0].length;
      continue;
    }
    const picture = found(PICTURE, text, i);
    const pictureTarget = picture === null ? null : found(TARGET, text, i + picture[0].length);
    if (picture !== null && pictureTarget !== null) {
      const written = picture[0] + pictureTarget[0];
      const src = pictureTarget[1] ?? pictureTarget[2];
      if (pictureFrom(src)) {
        flush();
        out.push({ kind: 'picture', src, alt: picture[1], title: pictureTarget[3] ?? null, ...marks });
      } else buf += written;
      i += written.length;
      continue;
    }
    const link = found(LINK, text, i);
    const linkTarget = link === null ? null : found(TARGET, text, i + link[0].length);
    if (link !== null && linkTarget !== null) {
      flush();
      // A link inside a link is its words.
      const href = marks.href ?? linkTo(linkTarget[1] ?? linkTarget[2]);
      const length = link[0].length + linkTarget[0].length;
      out.push(...inline(link[1], { ...marks, href }));
      i += length;
      continue;
    }
    const auto = found(AUTOLINK, text, i);
    if (auto !== null) {
      flush();
      out.push({ kind: 'text', text: auto[1], ...marks, href: marks.href ?? auto[1] });
      i += auto[0].length;
      continue;
    }
    // A run of marked words: the sign, words right after it, and the same sign closing them.
    const sign = found(SIGN, text, i)?.[0];
    if (sign !== undefined) {
      const opens = !/\s/.test(text[i + sign.length] ?? ' ') && !(sign[0] === '_' && WORDLIKE.test(text[i - 1] ?? ''));
      const end = opens ? closing(text, i + sign.length, sign) : -1;
      if (end < 0) {
        buf += sign;
        i += sign.length;
        continue;
      }
      flush();
      const set =
        sign === '~~'
          ? { ...marks, struck: true }
          : sign.length === 2
            ? { ...marks, strong: true }
            : { ...marks, stressed: true };
      out.push(...inline(text.slice(i + sign.length, end), set));
      i = end + sign.length;
      continue;
    }
    buf += c;
    i += 1;
  }
  flush();
  return out;
}

/// The name of each pointer of a passage that can hold its question's card, in its order, as often as it stands there:
/// those in paragraphs, of the passage, of an item of a list or of a quote, outside a link.
export const pointersIn = (text: string): string[] => heldIn(parse(text));

function heldIn(blocks: Block[]): string[] {
  return blocks.flatMap((b) => {
    switch (b.kind) {
      case 'paragraph':
        return b.pieces.flatMap((p) => (p.kind === 'pointer' && p.href === null ? [p.name] : []));
      case 'quote':
        return heldIn(b.blocks);
      case 'list':
        return b.items.flatMap(heldIn);
      default:
        return [];
    }
  });
}

/// What a passage draws at a pointer: the card of the question, at the first pointer to it, where the passage holds
/// it; else a link to it, where it is drawn elsewhere; else nothing, and the pointer is drawn as it was written.
export type Pointers = {
  hold: (name: string) => ReactNode | null;
  link: (name: string) => ReactNode | null;
};

const sameMarks = (a: Marks, b: Marks) =>
  a.strong === b.strong && a.stressed === b.stressed && a.struck === b.struck && a.href === b.href;

/// Words set as `marks` say, in the link they stand in.
function marked(marks: Marks, words: ReactNode[], key: number): ReactNode {
  let node: ReactNode = words;
  if (marks.struck) node = <s>{node}</s>;
  if (marks.stressed) node = <em>{node}</em>;
  if (marks.strong) node = <strong>{node}</strong>;
  if (marks.href === null) return <Fragment key={key}>{node}</Fragment>;
  // A link to the web opens in a tab of its own; one to a view of the page, in this one.
  const away = !marks.href.startsWith('#/');
  return (
    <a
      key={key}
      className="link"
      href={marks.href}
      target={away ? '_blank' : undefined}
      rel={away ? 'noreferrer' : undefined}
    >
      {node}
    </a>
  );
}

/// The words of a line, each run of pieces set alike in one part.
function words(pieces: Inline[], pointers: Pointers | undefined): ReactNode[] {
  const out: ReactNode[] = [];
  for (let i = 0; i < pieces.length; ) {
    const first = pieces[i];
    const run: ReactNode[] = [];
    for (; i < pieces.length && sameMarks(pieces[i], first); i++) {
      const p = pieces[i];
      if (p.kind === 'text') run.push(p.text);
      else if (p.kind === 'code') run.push(<code key={i}>{p.text}</code>);
      else if (p.kind === 'break') run.push(<br key={i} />);
      else if (p.kind === 'picture') {
        run.push(<img key={i} src={p.src} alt={p.alt} title={p.title ?? undefined} />);
      } else {
        // A pointer in a link is its words: a link to its question cannot stand inside another.
        const to = p.href === null ? pointers?.link(p.name) : null;
        run.push(<Fragment key={i}>{to ?? p.written}</Fragment>);
      }
    }
    out.push(marked(first, run, i));
  }
  return out;
}

/// Whether pieces say anything: more than blanks and breaks.
const says = (pieces: Inline[]) => pieces.some((p) => (p.kind === 'text' ? p.text.trim() !== '' : p.kind !== 'break'));

/// Draws the blocks of a passage. `held` is the questions it drew a card of: a pointer to one again is a link.
function drawBlocks(blocks: Block[], pointers: Pointers | undefined, held: Set<string>): ReactNode[] {
  const drawn: ReactNode[] = [];
  for (const block of blocks) {
    const key = drawn.length;
    switch (block.kind) {
      case 'code': {
        const diagram = diagramKind(block.lang);
        drawn.push(
          block.lang === 'diff' ? (
            <Diff key={key} text={block.text} folded />
          ) : diagram !== null ? (
            <Diagram key={key} kind={diagram} source={block.text} />
          ) : (
            <CodeBlock key={key} text={block.text} grammar={grammarNamed(block.lang)} />
          ),
        );
        break;
      }
      case 'heading':
        drawn.push(
          <p key={key} className="cw-md-head">
            {words(block.pieces, pointers)}
          </p>,
        );
        break;
      case 'rule':
        drawn.push(<hr key={key} />);
        break;
      case 'quote':
        drawn.push(
          <blockquote key={key} className="cw-quote">
            {drawBlocks(block.blocks, pointers, held)}
          </blockquote>,
        );
        break;
      case 'list':
        drawn.push(
          <ol key={key} start={block.start === 1 ? undefined : block.start}>
            {block.items.map((item, i) => (
              <li key={i}>{drawBlocks(item, pointers, held)}</li>
            ))}
          </ol>,
        );
        break;
      case 'table':
        drawn.push(
          <table key={key} className="table cw-md-table">
            <thead>
              <tr>
                {block.head.map((cell, c) => (
                  <th key={c} scope="col" className="label" style={{ textAlign: block.align[c] ?? undefined }}>
                    {words(cell, pointers)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, r) => (
                <tr key={r}>
                  {row.map((cell, c) => (
                    <td key={c} style={{ textAlign: block.align[c] ?? undefined }}>
                      {words(cell, pointers)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>,
        );
        break;
      case 'paragraph': {
        // A paragraph is ended before each card it holds and begun again after it, so that no card stands inside one.
        let line: Inline[] = [];
        const end = () => {
          if (says(line)) drawn.push(<p key={drawn.length}>{words(line, pointers)}</p>);
          line = [];
        };
        for (const piece of block.pieces) {
          const card =
            piece.kind === 'pointer' && piece.href === null && !held.has(piece.name)
              ? (pointers?.hold(piece.name) ?? null)
              : null;
          if (card === null || piece.kind !== 'pointer') {
            line.push(piece);
            continue;
          }
          held.add(piece.name);
          end();
          drawn.push(<Fragment key={drawn.length}>{card}</Fragment>);
        }
        end();
        break;
      }
    }
  }
  return drawn;
}

export function Markdown({ text, pointers }: { text: string; pointers?: Pointers }) {
  return <div className="cw-md">{drawBlocks(parse(text), pointers, new Set())}</div>;
}
