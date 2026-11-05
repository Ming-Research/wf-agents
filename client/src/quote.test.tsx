// Quoting a bubble into the box at the bottom of a conversation, apart from drawing: the words a quote puts in the box
// (`quoted`), which bubbles can be quoted and who a quote names, and the markdown the page draws such words as, a quote.
// That the owner's words reach the page and the agent as the box holds them is counted in quote.dom.test.tsx.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { ConversationEntry } from './api';
import { quotable, quoted, quotedBy } from './conversation';
import { Markdown, parse } from './markdown';
import { t } from './text';

const entry = (extra: Partial<ConversationEntry> = {}): ConversationEntry => ({
  source: 'message',
  id: 1,
  sender: 'engineer',
  recipient: 'owner',
  text: 'Which table?',
  at: 1,
  work: 5,
  round: null,
  ask: null,
  design_card: null,
  matter: null,
  read_at: null,
  waiting: false,
  pending: false,
  undelivered: false,
  answerable: false,
  fault: null,
  misses: null,
  ask_detail: null,
  stage: null,
  matter_card: null,
  words_state: null,
  policy: null,
  plan: null,
  forwards: [],
  images: [],
  ...extra,
});

describe('the words a quote puts in the box', () => {
  it('name who said them on a line of their own, then every line of what the bubble says after a quote mark', () => {
    expect(quoted('architect', 'Which table?\n\nOr both.', null)).toBe('> **architect**\n>\n> Which table?\n>\n> Or both.\n\n');
  });

  it('are what the owner selected when something is selected, else the whole', () => {
    expect(quoted('architect', 'Which table?\n\nOr both.', 'Or both.')).toBe('> **architect**\n>\n> Or both.\n\n');
    expect(quoted('architect', 'Which table?', null)).toBe('> **architect**\n>\n> Which table?\n\n');
    // A selection of nothing but space is no selection.
    expect(quoted('architect', 'Which table?', ' \n ')).toBe('> **architect**\n>\n> Which table?\n\n');
  });

  it('end in a blank line, so that what the owner writes next is a paragraph of its own', () => {
    expect(parse(`${quoted('architect', 'Which table?', null)}Both.`)).toEqual([
      {
        kind: 'quote',
        blocks: [
          { kind: 'para', text: '**architect**' },
          { kind: 'para', text: 'Which table?' },
        ],
      },
      { kind: 'para', text: 'Both.' },
    ]);
  });

  it('keep a quoted text of three lines or more that end no sentence from being taken for code', () => {
    // Unquoted, three such lines are code; under the name's own paragraph they are still the bubble's words.
    const said = 'one\ntwo\nthree';
    expect(parse(said)).toEqual([{ kind: 'code', text: said }]);
    expect(parse(quoted('architect', said, null))[0]).toEqual({
      kind: 'quote',
      blocks: [
        { kind: 'para', text: '**architect**' },
        { kind: 'code', text: said },
      ],
    });
  });
});

describe('who can be quoted, and named', () => {
  it('is a message of the owner or of an agent with words in it', () => {
    expect(quotable(entry())).toBe(true);
    expect(quotable(entry({ sender: 'owner', recipient: 'engineer', source: 'interjection' }))).toBe(true);
    expect(quotable(entry({ source: 'ask', sender: 'engineer' }))).toBe(true);
    expect(quotable(entry({ source: 'between', sender: 'q', recipient: 'p' }))).toBe(true);
  });

  it('is nothing the page says itself: no words, a ruling, the engine, a fault, a card or a live bubble', () => {
    expect(quotable(entry({ text: ' \n ' }))).toBe(false);
    expect(quotable(entry({ source: 'ruling', sender: 'owner' }))).toBe(false);
    expect(quotable(entry({ sender: 'engine' }))).toBe(false);
    expect(quotable(entry({ source: 'notice', sender: 'engine' }))).toBe(false);
    expect(quotable(entry({ source: 'notice', sender: 'wf-agents', fault: 'flow_anomaly' }))).toBe(false);
    expect(quotable(entry({ source: 'card' }))).toBe(false);
    for (const source of ['stage', 'matter', 'policy', 'plan']) expect(quotable(entry({ source }))).toBe(false);
  });

  it('names the owner as owner, a line as itself, the policy line of the architect as the architect', () => {
    expect(quotedBy(entry({ sender: 'owner' }))).toBe('owner');
    expect(quotedBy(entry({ sender: 'engineer' }))).toBe('engineer');
    expect(quotedBy(entry({ sender: 'policy' }))).toBe('architect');
    expect(quotedBy(entry({ source: 'between', sender: 'q' }))).toBe(t('conv_architect_of', { project: 'q' }));
  });
});

describe('quoted lines in a text', () => {
  it('are one quote, whose own blocks are what is under the marks, and a quote in a quote is one', () => {
    expect(parse('> one\n>\n> - a\n> - b\n\nafter')).toEqual([
      {
        kind: 'quote',
        blocks: [
          { kind: 'para', text: 'one' },
          {
            kind: 'list',
            ordered: false,
            start: 1,
            items: [[{ kind: 'para', text: 'a' }], [{ kind: 'para', text: 'b' }]],
          },
        ],
      },
      { kind: 'para', text: 'after' },
    ]);
    expect(parse('> > inner\n> outer')).toEqual([
      {
        kind: 'quote',
        blocks: [
          { kind: 'quote', blocks: [{ kind: 'para', text: 'inner' }] },
          { kind: 'para', text: 'outer' },
        ],
      },
    ]);
  });

  it('end a paragraph or a list they follow, and end where a line without the mark begins', () => {
    expect(parse('before\n> quoted\nafter')).toEqual([
      { kind: 'para', text: 'before' },
      { kind: 'quote', blocks: [{ kind: 'para', text: 'quoted' }] },
      { kind: 'para', text: 'after' },
    ]);
    expect(parse('- item\n> quoted')).toEqual([
      { kind: 'list', ordered: false, start: 1, items: [[{ kind: 'para', text: 'item' }]] },
      { kind: 'quote', blocks: [{ kind: 'para', text: 'quoted' }] },
    ]);
  });

  it('are not a mark in the middle of a line, nor in a fence', () => {
    expect(parse('a > b')).toEqual([{ kind: 'para', text: 'a > b' }]);
    expect(parse('```\n> not a quote\n```')).toEqual([{ kind: 'code', text: '> not a quote' }]);
  });

  it('are drawn as a quote under its edge, with the words and not the marks', () => {
    const html = renderToStaticMarkup(<Markdown text={`${quoted('architect', 'Which table?', null)}Both.`} />);
    expect(html).toContain('<blockquote class="md-quote">');
    expect(html).toContain('<strong>architect</strong>');
    expect(html).toContain('Which table?');
    expect(html).not.toContain('&gt;');
    // What the owner wrote is outside the quote.
    expect(html.indexOf('</blockquote>')).toBeLessThan(html.indexOf('Both.'));
  });
});
