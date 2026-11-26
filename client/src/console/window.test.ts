import { describe, expect, it } from 'vitest';

import { paragraphsOf, passagesOf, replySeen } from './window';

describe('the paragraphs of a text', () => {
  it('are the runs of lines between blank lines, however many blank lines part them', () => {
    expect(paragraphsOf('One.\nStill one.\n\n\nTwo.\n\n  \nThree.')).toEqual(['One.\nStill one.', 'Two.', 'Three.']);
    expect(paragraphsOf('')).toEqual([]);
  });

  it('keep a fenced block whole, the blank lines in it too', () => {
    const text = 'Before.\n\n```mermaid\nflowchart LR\n\n  A --> B\n```\n\nAfter.';
    expect(paragraphsOf(text)).toEqual(['Before.', '```mermaid\nflowchart LR\n\n  A --> B\n```', 'After.']);
  });
});

describe('a text as the passages the owner has not read', () => {
  const read = 'The problem.\n\nThe plan, as it was.\n\nThe last word.';

  it('marks changed each run of paragraphs the text read before has no equal of, and the paragraphs between them not', () => {
    const now = 'The problem.\n\nThe plan, as it is now.\n\nA new paragraph.\n\nThe last word.';
    expect(passagesOf(now, read)).toEqual([
      { kind: 'text', text: 'The problem.' },
      { kind: 'text', text: 'The plan, as it is now.\n\nA new paragraph.', changed: true },
      { kind: 'text', text: 'The last word.' },
    ]);
  });

  it('marks nothing where the text is what was read, and gives it whole, as written', () => {
    const text = 'The problem.\n\n\nThe plan, as it was.\n\nThe last word.';
    expect(passagesOf(text, read)).toEqual([{ kind: 'text', text }]);
  });

  it('marks nothing where nothing was read before: an item read of nothing is new as a whole', () => {
    expect(passagesOf(read, null)).toEqual([{ kind: 'text', text: read }]);
  });

  it('takes a paragraph moved or spaced differently for the same paragraph, and a paragraph words changed for another', () => {
    expect(passagesOf('The last word.  \n\nThe problem.', read)).toEqual([{ kind: 'text', text: 'The last word.  \n\nThe problem.' }]);
    expect(passagesOf('The problem.\n\nThe last word!', read)).toEqual([
      { kind: 'text', text: 'The problem.' },
      { kind: 'text', text: 'The last word!', changed: true },
    ]);
  });
});

describe('a reply in view of the window', () => {
  const pane = { top: 100, bottom: 500 };

  it('is seen where all of it is within the pane, and not where any of it is out', () => {
    expect(replySeen({ top: 120, bottom: 300 }, pane)).toBe(true);
    expect(replySeen({ top: 50, bottom: 300 }, pane)).toBe(false);
    expect(replySeen({ top: 400, bottom: 560 }, pane)).toBe(false);
  });

  it('is seen, where it is taller than the pane, once its end is in view, and not before', () => {
    expect(replySeen({ top: 100, bottom: 900 }, pane)).toBe(false);
    expect(replySeen({ top: -300, bottom: 500 }, pane)).toBe(true);
    // Its end has gone above the pane.
    expect(replySeen({ top: -900, bottom: 100 }, pane)).toBe(false);
  });

  it('is not seen where it has no height, as one not drawn has none', () => {
    expect(replySeen({ top: 0, bottom: 0 }, pane)).toBe(false);
  });
});
