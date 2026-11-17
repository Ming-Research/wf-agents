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

  it('is a later text where an earlier one points to it only inside a fence it never closes, each text read apart', () => {
    // Read as one passage, the open fence would swallow the pointer of `two`; with no unclosed fence, `one` would hold it.
    const texts = [
      { key: 'one', text: '```\n{{a}}' },
      { key: 'two', text: 'See {{a}}.' },
    ];
    expect([...heldAt(texts, new Set(['a']))]).toEqual([['a', 'two']]);
  });
});

describe('the pointers of a passage, against the marks around them', () => {
  it('are none where strong marks cut the braces, and are kept where strong marks wrap a whole pointer', () => {
    // An implementation that found `{{...}}` in the raw text before the strong marks would read `sc**ope` as a name.
    expect(pointersIn('**Pick {{sc**ope}}')).toEqual([]);
    expect(pointersIn('**{{a}}** and **b {{c}} d**')).toEqual(['a', 'c']);
  });

  it('are none in a fence that never closes, to the end of the passage, its blank lines and all', () => {
    // An implementation that ended a fence at a blank line, or read no fence without its close, would find `b` or `c`.
    expect(pointersIn('Before {{a}}\n\n```mermaid\n{{b}}\n\nStill code {{c}}')).toEqual(['a']);
  });

  it('read a fence that opens right under a line of a paragraph, and the paragraph right after its close', () => {
    // An implementation that took a fence only after a blank line would read `{{b}}` as prose.
    expect(pointersIn('p {{a}}\n```\n{{b}}\n```\nq {{c}}')).toEqual(['a', 'c']);
  });

  it('stand right against code spans, and none is made of braces that code spans hold apart', () => {
    // An implementation that looked for pointers before code spans would read "` b `" between the spans as a name.
    expect(pointersIn('`x`{{a}}`y` and `{{` b `}}` and {{c}}')).toEqual(['a', 'c']);
  });

  it('keep a name of markup, quotes, slashes, spaces and the characters of regular expressions as it is written', () => {
    // An implementation that escaped, trimmed or cut a name at such a character would name another question.
    const names = [`a <b>"c'd</b> / e`, 'x.y*z+(w)?[v]$^|\\'];
    expect(pointersIn(names.map((n) => `{{${n}}}`).join(' then '))).toEqual(names);
  });
});
