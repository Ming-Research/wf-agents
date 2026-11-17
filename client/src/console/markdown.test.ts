import { describe, expect, it } from 'vitest';

import { pointersIn } from './markdown';
import { heldAt } from './window';

describe('the pointers of a passage to the questions of its work', () => {
  it('are each `{{name}}` of its prose and strong words, in their order, as often as they stand, the name as written', () => {
    const text = 'See {{a}} first,\nthen **{{b}} and {{a}}**.\n\n{{ c }} and {{d-e_f}}';
    expect(pointersIn(text)).toEqual(['a', 'b', 'a', ' c ', 'd-e_f']);
  });

  it('are none in code, in backticks or between fences, nor in braces that are not doubled or not closed', () => {
    const text = 'Code: `{{a}}` and `${{ x }}`.\n\n```\n{{b}}\n```\n\nNot one: {c}, {{d}, {{}}.';
    expect(pointersIn(text)).toEqual([]);
  });
});

describe('where a question a text points to is held', () => {
  it('is the first text, in the order given, that points to it; a name of no question is held nowhere', () => {
    const texts = [
      { key: 'one', text: 'Nothing here.' },
      { key: 'two', text: 'See {{b}} and {{a}}.' },
      { key: 'three', text: '{{a}} again, {{c}}, and {{ghost}}.' },
    ];
    expect([...heldAt(texts, new Set(['a', 'b', 'c']))]).toEqual([
      ['b', 'two'],
      ['a', 'two'],
      ['c', 'three'],
    ]);
  });

  it('is nowhere for a pointer in code', () => {
    expect([...heldAt([{ key: 'one', text: '`{{a}}`' }], new Set(['a']))]).toEqual([]);
  });
});
