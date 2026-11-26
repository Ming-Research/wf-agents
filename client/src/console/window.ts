// What the window beside the console shows of an item beyond what the interface gives: its body (a section for each
// datum its flow's card names, with the choices that wait for the owner where they stand in it), the messages under
// it, the owner's and the replies, and where the questions put to the owner stand among them. Pure, so that the window
// draws made-up items as it will draw real ones.

import { pointersIn } from './markdown';

/// Who writes a message, or is named in one to receive it, besides a cell or a line of the flow.
export type Who = 'owner' | 'architect' | 'assistant';

/// A cell of the engineer's flow a message can go to, or come from, by its id: the research (`R1`), the deep research
/// (`D1`), the implementation (`I1`), or a cell that asked the owner a question while working.
export type MessageCell = string;

/// A passage of the body, written in markdown. Changed when it changed since the owner last marked the item read.
export type Passage = { kind: 'text'; text: string; changed?: boolean };

/// A part of the body: a passage, or a choice that waits for the owner where it stands in the text, which the owner
/// may already have confirmed while others still wait. A choice a cell asked while working (`asks`) is answered to the
/// version of the work's question of that name; it holds the work (`holds`) where the engine's one list of what waits
/// on the owner names it, at the cell that asked it or at the merge.
export type Part =
  | Passage
  | {
      kind: 'choice';
      question: string;
      options: { label: string; detail: string }[];
      recommended: number | null;
      chosen: number | null;
      confirmed?: boolean;
      changed?: boolean;
      asks?: { cell: string; question: string; version: number };
      holds?: boolean;
      /// Why the cell that asked it does not run now, where its condition does not hold: the question holds the work at
      /// no cell.
      skipped?: string;
      /// Its answer goes into the design tree: the question is marked `tree`.
      tree?: boolean;
    };

export type Choice = Extract<Part, { kind: 'choice' }>;

/// A message under the body: who wrote it, when, to whom, what of the body it quotes, and what it says. A cell writes
/// one, or is written to, by its id.
export type Message = {
  who: Who | MessageCell;
  at: number;
  to: (Who | MessageCell)[];
  quote: string | null;
  text: string;
  /// Of the owner's words to a cell: whether a run of that cell has begun with them; of any other message, none.
  taken?: boolean;
  /// Of a reply of a cell that the engine counts as one: the block and the number of the version that holds it, and
  /// whether the owner has read it; of any other message, none.
  reply?: { block: string; version: number; read: boolean };
};

/// A version of a block of the work, by its block and its number among the block's versions, as the engine names it.
export type ReadVersion = { block: string; version: number };

/// A section of the body: a datum the flow's card names, by its name, with the passages of its current version.
export type BodySection = { title: string; parts: Part[] };

/// The item's body and messages.
export type Body = {
  /// What the flow's card names, in its order.
  sections: BodySection[];
  /// Whether the choices of the last section wait for the owner to confirm them.
  choicesWait: boolean;
  messages: Message[];
  /// The versions the owner reads in reading the item's body: the latest version a run wrote of each datum the card
  /// names, where the owner has not read it. None where the body is made up.
  toRead?: ReadVersion[];
};

/// A text's paragraphs: the runs of lines between blank lines, a fenced block kept whole with the blank lines in it.
export function paragraphsOf(text: string): string[] {
  const out: string[] = [];
  let lines: string[] = [];
  let fenced = false;
  const end = () => {
    if (lines.length > 0) out.push(lines.join('\n'));
    lines = [];
  };
  for (const line of text.split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
    if (!fenced && line.trim() === '') end();
    else lines.push(line);
  }
  end();
  return out;
}

/// A text as passages against the text the owner last read: each run of its paragraphs that the read text has no equal
/// of is one passage marked changed, the paragraphs between them passages not marked. With nothing read before, or
/// nothing changed, the text is one passage unmarked: an item the owner has read nothing of is new as a whole.
export function passagesOf(text: string, read: string | null): Passage[] {
  if (read === null) return [{ kind: 'text', text }];
  const had = new Set(paragraphsOf(read).map((p) => p.trim()));
  const passages: Passage[] = [];
  let changes = 0;
  for (const paragraph of paragraphsOf(text)) {
    const changed = !had.has(paragraph.trim());
    const last = passages[passages.length - 1];
    if (changed) changes += 1;
    if (last !== undefined && (last.changed === true) === changed) last.text += `\n\n${paragraph}`;
    else passages.push({ kind: 'text', text: paragraph, ...(changed ? { changed } : {}) });
  }
  return changes === 0 ? [{ kind: 'text', text }] : passages;
}

/// Whether a reply drawn in the box `reply` is in view of the pane `pane`: all of it where it fits in the pane, and, where
/// it is taller than the pane, its end, which is where the owner has read it to. A box with no height is not drawn.
export function replySeen(
  reply: { top: number; bottom: number },
  pane: { top: number; bottom: number },
  slack = 1,
): boolean {
  const height = reply.bottom - reply.top;
  if (height <= 0) return false;
  if (height <= pane.bottom - pane.top) return reply.top >= pane.top - slack && reply.bottom <= pane.bottom + slack;
  return reply.bottom <= pane.bottom + slack && reply.bottom > pane.top + slack;
}

/// How many parts of the body changed since the owner last read the item.
export const changedIn = (b: Body): number =>
  b.sections.flatMap((s) => s.parts).filter((p) => p.changed === true).length;

/// A question as a message quotes it to discuss it: its sentence, then each option's label, each on a line.
export const quoteOf = (c: Choice): string => [c.question, ...c.options.map((o) => o.label)].join('\n');

/// The messages about a question: those that quote its sentence.
export const messagesAbout = (messages: Message[], c: Choice): number[] =>
  messages.flatMap((n, i) => (n.quote !== null && n.quote.includes(c.question) ? [i] : []));

/// How many choices the last section has, and how many of them have their pick.
export function picks(b: Body): { picked: number; all: number } {
  const choices = (b.sections.at(-1)?.parts ?? []).filter((p) => p.kind === 'choice');
  return { picked: choices.filter((p) => p.chosen !== null).length, all: choices.length };
}

/// A text of the window that may hold a question it points to: by a key of where it stands, and what it says.
export type Pointing = { key: string; text: string };

/// Where each question that a text points to is drawn: at the first pointer to it, the texts read in the order given;
/// by the question's name, the key of the text that holds it. A pointer to none of `names` holds nothing.
export function heldAt(texts: Pointing[], names: ReadonlySet<string>): Map<string, string> {
  const at = new Map<string, string>();
  for (const t of texts) {
    for (const name of pointersIn(t.text)) if (names.has(name) && !at.has(name)) at.set(name, t.key);
  }
  return at;
}
