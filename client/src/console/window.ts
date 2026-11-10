// What the window beside the console shows of an item beyond what the interface gives: its body (where it came from,
// the problem it solves and the plan for it, with the choices that wait for the owner where they stand in it), and the
// notes under it, the owner's and the replies. Pure, so that the window draws made-up items as it will draw real ones.
//
// Brought over from the console of the engine on `main` (`client/src/console/window.ts`), as it was.

/// Who writes a note, or is named in one to receive it.
export type Who = 'owner' | 'engineer' | 'architect' | 'assistant';

/// A part of the body: a passage written in markdown, or a choice that waits for the owner where it stands in the
/// text. Changed when it changed since the owner last marked the item read.
export type Part =
  | { kind: 'text'; text: string; changed?: boolean }
  | {
      kind: 'choice';
      question: string;
      options: { label: string; detail: string }[];
      recommended: number | null;
      chosen: number | null;
      changed?: boolean;
    };

/// A note under the body: who wrote it, when, to whom, what of the body it quotes, and what it says.
export type Note = { who: Who; at: number; to: Who[]; quote: string | null; text: string };

/// The item's body and notes.
export type Body = {
  background: Part[];
  problem: Part[];
  plan: Part[];
  /// Whether the plan waits for the owner to confirm it.
  planWaits: boolean;
  notes: Note[];
};

/// How many parts of the body changed since the owner last read the item.
export const changedIn = (b: Body): number =>
  [...b.background, ...b.problem, ...b.plan].filter((p) => p.changed === true).length;

/// How many choices the plan has, and how many of them have their pick.
export function picks(b: Body): { picked: number; all: number } {
  const choices = b.plan.filter((p) => p.kind === 'choice');
  return { picked: choices.filter((p) => p.chosen !== null).length, all: choices.length };
}

/// Those named in a note with `@`; the engineer only while a work goes on.
export function named(text: string, working: boolean): Who[] {
  const found = new Set<Who>();
  for (const m of text.matchAll(/@(engineer|architect|assistant)\b/gi)) found.add(m[1].toLowerCase() as Who);
  if (!working) found.delete('engineer');
  return [...found];
}
