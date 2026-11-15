// What the window beside the console shows of an item beyond what the interface gives: its body (where it came from,
// the problem it solves and the plan for it, with the choices that wait for the owner where they stand in it, and the
// questions a cell asked while working at its end), and the messages under it, the owner's and the replies. Pure, so
// that the window draws made-up items as it will draw real ones.
//
// Brought over from the console of the engine on `old-main` (`client/src/console/window.ts`), as it was.

/// Who writes a message, or is named in one to receive it.
export type Who = 'owner' | 'engineer' | 'architect' | 'assistant';

/// A cell of the engineer's flow a message can go to, or come from, by its id: the research (`R1`), the deep research
/// (`D1`), the implementation (`I1`), or a cell that asked the owner a question while working.
export type MessageCell = string;

/// A passage of the body, written in markdown. Changed when it changed since the owner last marked the item read.
export type Passage = { kind: 'text'; text: string; changed?: boolean };

/// A part of the body: a passage, or a choice that waits for the owner where it stands in the text, which the owner
/// may already have confirmed while others still wait. A choice a cell asked while working (`asks`) is answered to the
/// call that waits for it, or to the version of the work's question of that name; it holds the work (`holds`) where
/// the engine's one list of what waits on the owner names it, at the cell that asked it or at the merge.
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
      asks?: { cell: string; call: number } | { cell: string; question: string; version: number };
      holds?: boolean;
      /// Why a cell other than the one that asked it waits for it, in the engine's sentence, said under what it asks:
      /// as of a question the merge waits for (`Wait.under`).
      under?: string;
      /// What the owner wrote as the answer to a question with no options, which is answered in words.
      answer?: string;
    };

export type Choice = Extract<Part, { kind: 'choice' }>;

/// A message under the body: who wrote it, when, to whom, what of the body it quotes, and what it says. A cell writes
/// one, or is written to, by its id.
export type Message = { who: Who | MessageCell; at: number; to: (Who | MessageCell)[]; quote: string | null; text: string };

/// The item's body and messages.
export type Body = {
  background: Part[];
  problem: Part[];
  plan: Part[];
  /// Whether the plan waits for the owner to confirm it.
  planWaits: boolean;
  messages: Message[];
};

/// How many parts of the body changed since the owner last read the item.
export const changedIn = (b: Body): number =>
  [...b.background, ...b.problem, ...b.plan].filter((p) => p.changed === true).length;

/// A question as a message quotes it to discuss it: its sentence, then each option's label, each on a line.
export const quoteOf = (c: Choice): string => [c.question, ...c.options.map((o) => o.label)].join('\n');

/// The messages about a question: those that quote its sentence.
export const messagesAbout = (messages: Message[], c: Choice): number[] =>
  messages.flatMap((n, i) => (n.quote !== null && n.quote.includes(c.question) ? [i] : []));

/// How many choices the plan has, and how many of them have their pick.
export function picks(b: Body): { picked: number; all: number } {
  const choices = b.plan.filter((p) => p.kind === 'choice');
  return { picked: choices.filter((p) => p.chosen !== null).length, all: choices.length };
}

/// Those named in a message with `@`; the engineer only while a work goes on.
export function named(text: string, working: boolean): Who[] {
  const found = new Set<Who>();
  for (const m of text.matchAll(/@(engineer|architect|assistant)\b/gi)) found.add(m[1].toLowerCase() as Who);
  if (!working) found.delete('engineer');
  return [...found];
}
