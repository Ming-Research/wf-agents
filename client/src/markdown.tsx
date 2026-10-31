// Text the agents write, as it reads: the markdown they write in it (paragraphs, lists, headings, fenced code, inline
// code, bold and links) drawn as such, and a paragraph that is plainly code, such as a command to run, kept as code.
// A small parser of its own, since the page needs only these, and what an agent writes must never become markup
// the page did not mean: nothing in the text is taken as HTML.

import type { ReactNode } from 'react';

export type Inline =
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'bold'; children: Inline[] }
  | { kind: 'link'; href: string; children: Inline[] };

export type Block =
  | { kind: 'para'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'heading'; level: number; text: string }
  | { kind: 'list'; ordered: boolean; start: number; items: Block[][] };

const FENCE = /^ {0,3}(```|~~~)/;
const HEADING = /^ {0,3}(#{1,6})[ \t]+(.*?)[ \t#]*$/;
const ITEM = /^( *)([-*+]|\d{1,9}[.)])(?:[ \t]+(.*))?$/;

const blank = (l: string) => l.trim() === '';
const indentOf = (l: string) => l.length - l.trimStart().length;
const ordered = (marker: string) => /\d/.test(marker);
/// A sentence ends with a character Unicode names a sentence terminal, in any language.
const endsSentence = (l: string) => /\p{Sentence_Terminal}\s*$/u.test(l);

/// The blocks of a text.
export function parse(text: string): Block[] {
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
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].trimStart().startsWith(fence[1])) {
        body.push(lines[i]);
        i += 1;
      }
      i += 1;
      const code = body.join('\n').replace(/^\n+|\n+$/g, '');
      if (code !== '') out.push({ kind: 'code', text: code });
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading !== null) {
      out.push({ kind: 'heading', level: heading[1].length, text: heading[2] });
      i += 1;
      continue;
    }
    if (ITEM.test(line)) {
      i = list(lines, i, out);
      continue;
    }
    const para: string[] = [];
    while (i < lines.length && !blank(lines[i]) && !FENCE.test(lines[i]) && !HEADING.test(lines[i]) && !ITEM.test(lines[i])) {
      para.push(lines[i]);
      i += 1;
    }
    // Three lines or more none of which ends a sentence are code written without fences, such as a command.
    const code = para.length >= 3 && !para.some(endsSentence);
    out.push(code ? { kind: 'code', text: para.join('\n') } : { kind: 'para', text: para.join('\n') });
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
    if (m !== null && m[1].length <= indent && m[1].length >= indent - 1 && ordered(m[2]) === isOrdered) {
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
    if (!afterBlank && m === null && !FENCE.test(line) && !HEADING.test(line)) {
      item.push(line.trimStart());
      i += 1;
      continue;
    }
    break;
  }
  const n = isOrdered ? Number.parseInt(first[2], 10) : 1;
  out.push({ kind: 'list', ordered: isOrdered, start: n, items: items.map(blocksOf) });
  return i;
}

/// The inline parts of a paragraph: code between backticks, bold between double asterisks, links, and text.
export function inline(text: string): Inline[] {
  const out: Inline[] = [];
  let buf = '';
  const flush = () => {
    if (buf !== '') out.push({ kind: 'text', text: buf });
    buf = '';
  };
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (c === '\\' && i + 1 < text.length && /[\\`*[\]]/.test(text[i + 1])) {
      buf += text[i + 1];
      i += 2;
      continue;
    }
    if (c === '`') {
      let n = 1;
      while (text[i + n] === '`') n += 1;
      const ticks = '`'.repeat(n);
      const end = text.indexOf(ticks, i + n);
      if (end > i) {
        flush();
        const code = text.slice(i + n, end);
        out.push({ kind: 'code', text: /^ .*\S.* $/.test(code) ? code.slice(1, -1) : code });
        i = end + n;
        continue;
      }
      buf += ticks;
      i += n;
      continue;
    }
    if (text.startsWith('**', i)) {
      const end = text.indexOf('**', i + 2);
      if (end > i + 2) {
        flush();
        out.push({ kind: 'bold', children: inline(text.slice(i + 2, end)) });
        i = end + 2;
        continue;
      }
    }
    if (c === '[') {
      const m = /^\[([^\]\n]+)\]\(([^()\s]+)\)/.exec(text.slice(i));
      if (m !== null) {
        flush();
        out.push({ kind: 'link', href: m[2], children: inline(m[1]) });
        i += m[0].length;
        continue;
      }
    }
    buf += c;
    i += 1;
  }
  flush();
  return out;
}

/// Only a link to the web leaves the page; any other target an agent wrote, such as a path in its repository, is
/// shown as its words.
const WEB = /^https?:\/\//;

function Inlines({ parts }: { parts: Inline[] }): ReactNode {
  return parts.map((p, i) => {
    switch (p.kind) {
      case 'text':
        return p.text;
      case 'code':
        return <code key={i}>{p.text}</code>;
      case 'bold':
        return (
          <strong key={i}>
            <Inlines parts={p.children} />
          </strong>
        );
      case 'link':
        return WEB.test(p.href) ? (
          <a key={i} href={p.href} target="_blank" rel="noreferrer">
            <Inlines parts={p.children} />
          </a>
        ) : (
          <span key={i}>
            <Inlines parts={p.children} />
          </span>
        );
    }
    return null;
  });
}

function Blocks({ blocks }: { blocks: Block[] }) {
  return blocks.map((b, i) => {
    switch (b.kind) {
      case 'para':
        return (
          <p key={i} className="prose">
            <Inlines parts={inline(b.text)} />
          </p>
        );
      case 'code':
        return (
          <pre key={i} className="code-block">
            {b.text}
          </pre>
        );
      case 'heading':
        return (
          <p key={i} className={`md-h md-h${Math.min(b.level, 3)}`}>
            <Inlines parts={inline(b.text)} />
          </p>
        );
      case 'list': {
        const items = b.items.map((blocks, j) => (
          <li key={j}>
            {/* An item of one paragraph is its text alone, so that a list of short items stays tight. */}
            {blocks.length === 1 && blocks[0].kind === 'para' ? (
              <Inlines parts={inline(blocks[0].text)} />
            ) : (
              <Blocks blocks={blocks} />
            )}
          </li>
        ));
        return b.ordered ? (
          <ol key={i} className="md-list" start={b.start}>
            {items}
          </ol>
        ) : (
          <ul key={i} className="md-list">
            {items}
          </ul>
        );
      }
    }
    return null;
  });
}

/// A text an agent wrote, as it reads.
export function Markdown({ text }: { text: string }) {
  return (
    <div className="md">
      <Blocks blocks={parse(text)} />
    </div>
  );
}

/// A short text an agent wrote that runs on with what is beside it, such as a claim after its badge: one paragraph as
/// its inline parts; anything longer as blocks.
export function MarkdownInline({ text }: { text: string }) {
  const blocks = parse(text);
  if (blocks.length === 1 && blocks[0].kind === 'para') {
    return (
      <span className="prose">
        <Inlines parts={inline(blocks[0].text)} />
      </span>
    );
  }
  return <Markdown text={text} />;
}
