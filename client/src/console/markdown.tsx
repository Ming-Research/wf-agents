// A passage of markdown as the item window shows it: paragraphs, blocks of code between fences (a diagram among them,
// shown as its source: the page has no library that draws one), and in a line `code`, **strong** words and a pointer
// to a question of the work, `{{name}}`, as the words of a cell's questions tell its agent to write one (decision
// a-question-stands-where-a-text-points-to-it). What stands in code is text, a pointer too.

import { Fragment, type ReactNode } from 'react';

/// A piece of a line: words, code, or a pointer to the question of this name, as it was written; strong where it
/// stands between `**`.
type Inline = (
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'pointer'; name: string; written: string }
) & { strong: boolean };

/// A block of a passage: a paragraph of pieces, or the lines of code between fences.
type Block = { kind: 'paragraph'; pieces: Inline[] } | { kind: 'code'; text: string };

/// The pieces of the words of a paragraph, or of its strong words.
function inline(text: string, strong: boolean): Inline[] {
  const out: Inline[] = [];
  const re = /`([^`]+)`|\*\*([^*]+)\*\*|\{\{([^{}]+)\}\}/g;
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > at) out.push({ kind: 'text', text: text.slice(at, m.index), strong });
    if (m[1] !== undefined) out.push({ kind: 'code', text: m[1], strong });
    else if (m[2] !== undefined) out.push(...inline(m[2], true));
    else out.push({ kind: 'pointer', name: m[3], written: m[0], strong });
    at = m.index + m[0].length;
  }
  if (at < text.length) out.push({ kind: 'text', text: text.slice(at), strong });
  return out;
}

/// The blocks of a passage: its lines between fences, and its other lines in paragraphs, which a blank line ends.
function parse(text: string): Block[] {
  const blocks: Block[] = [];
  const lines = text.split('\n');
  let para: string[] = [];
  const flush = () => {
    if (para.length > 0) blocks.push({ kind: 'paragraph', pieces: inline(para.join(' '), false) });
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith('```')) {
      flush();
      const code: string[] = [];
      for (i += 1; i < lines.length && !lines[i].startsWith('```'); i++) code.push(lines[i]);
      blocks.push({ kind: 'code', text: code.join('\n') });
    } else if (line.trim() === '') flush();
    else para.push(line);
  }
  flush();
  return blocks;
}

/// The name of each pointer of a passage, in its order, as often as it stands there.
export const pointersIn = (text: string): string[] =>
  parse(text).flatMap((b) =>
    b.kind === 'paragraph' ? b.pieces.flatMap((p) => (p.kind === 'pointer' ? [p.name] : [])) : [],
  );

/// What a passage draws at a pointer: the card of the question, at the first pointer to it, where the passage holds
/// it; else a link to it, where it is drawn elsewhere; else nothing, and the pointer is drawn as it was written.
export type Pointers = {
  hold: (name: string) => ReactNode | null;
  link: (name: string) => ReactNode | null;
};

/// The words of a line, each run of strong pieces in one strong part.
function words(pieces: Inline[], pointers: Pointers | undefined): ReactNode[] {
  const out: ReactNode[] = [];
  for (let i = 0; i < pieces.length; ) {
    const strong = pieces[i].strong;
    const run: ReactNode[] = [];
    for (; i < pieces.length && pieces[i].strong === strong; i++) {
      const p = pieces[i];
      if (p.kind === 'text') run.push(p.text);
      else if (p.kind === 'code') run.push(<code key={i}>{p.text}</code>);
      else run.push(<Fragment key={i}>{pointers?.link(p.name) ?? p.written}</Fragment>);
    }
    out.push(strong ? <strong key={i}>{run}</strong> : <Fragment key={i}>{run}</Fragment>);
  }
  return out;
}

/// Whether pieces say anything: more than blanks.
const says = (pieces: Inline[]) => pieces.some((p) => p.kind !== 'text' || p.text.trim() !== '');

export function Markdown({ text, pointers }: { text: string; pointers?: Pointers }) {
  const drawn: ReactNode[] = [];
  // The questions this passage drew a card of: a pointer to one again is a link.
  const held = new Set<string>();
  for (const block of parse(text)) {
    if (block.kind === 'code') {
      drawn.push(
        <pre key={drawn.length} className="content">
          <code>{block.text}</code>
        </pre>,
      );
      continue;
    }
    // A paragraph is ended before each card it holds and begun again after it, so that no card stands inside one.
    let line: Inline[] = [];
    const end = () => {
      if (says(line)) drawn.push(<p key={drawn.length}>{words(line, pointers)}</p>);
      line = [];
    };
    for (const piece of block.pieces) {
      const card = piece.kind === 'pointer' && !held.has(piece.name) ? (pointers?.hold(piece.name) ?? null) : null;
      if (card === null || piece.kind !== 'pointer') {
        line.push(piece);
        continue;
      }
      held.add(piece.name);
      end();
      drawn.push(<Fragment key={drawn.length}>{card}</Fragment>);
    }
    end();
  }
  return <div className="cw-md">{drawn}</div>;
}
