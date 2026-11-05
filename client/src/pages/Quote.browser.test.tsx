// The button that quotes a bubble, in a real browser (Chromium, through Vitest's browser mode): it shows only while the
// pointer is on the bubble, and a press of it quotes the words the owner selected in that bubble, or the whole bubble
// when nothing is selected in it, which depends on the browser's own selection and pointer, not on a DOM made up by a
// test. The page is the real stream of a conversation (`Stream`, with the page's own styles and Primer's theme); what
// is checked is what the button hands to the box, which quote.dom.test.tsx counts after that.

import '@primer/primitives/dist/css/primitives.css';
import '@primer/primitives/dist/css/functional/themes/light.css';
import '@primer/primitives/dist/css/functional/themes/dark.css';
import '../page-styles';

import { BaseStyles } from '@primer/react';
import { ThemeProvider } from '@primer/react/next';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';

import { AnswersHere } from '../answer';
import type { Conversation, ConversationDetail, ConversationEntry } from '../api';
import { Stream } from './Messages';

const entry = (id: number, text: string): ConversationEntry => ({
  source: 'message',
  id,
  sender: 'engineer',
  recipient: 'owner',
  text,
  at: id,
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
});

const conversation: Conversation = {
  key: 'work:5',
  kind: 'work',
  project: 'p',
  projects: [],
  work: 5,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: 1,
};

const detail: ConversationDetail = {
  key: 'work:5',
  entries: [entry(1, 'Which table?\n\nOr both.'), entry(2, 'Another message.')],
  asks: [],
  design_cards: [],
  questions: [],
  waits_on: null,
  forwarding: null,
  read_to: 1_000_000,
  gaps: [],
};

const quote = vi.fn<(e: ConversationEntry, selected: string | null) => void>();

beforeEach(async () => {
  quote.mockClear();
  const root = document.documentElement;
  root.setAttribute('data-color-mode', 'auto');
  root.setAttribute('data-light-theme', 'light');
  root.setAttribute('data-dark-theme', 'dark');
  await render(
    <ThemeProvider colorMode="auto">
      <BaseStyles>
        <AnswersHere.Provider value={true}>
          <Stream d={detail} c={conversation} w={null} quote={quote} />
        </AnswersHere.Provider>
      </BaseStyles>
    </ThemeProvider>,
  );
});

afterEach(() => window.getSelection()?.removeAllRanges());

/// The bubble of the message with this order in the stream, and its button.
function bubble(n: number): { bubble: HTMLElement; button: HTMLButtonElement; words: HTMLElement } {
  const found = document.querySelectorAll<HTMLElement>('.talk-bubble')[n];
  if (found === undefined) throw new Error(`the stream has no bubble ${n}`);
  const button = found.querySelector<HTMLButtonElement>('.talk-quote');
  const words = found.querySelector<HTMLElement>('.talk-bubble-body p');
  if (button === null || words === null) throw new Error(`bubble ${n} has no button or no words`);
  return { bubble: found, button, words };
}

const opacity = (el: Element) => getComputedStyle(el).opacity;

describe('the button that quotes a bubble, in a real browser', () => {
  it('shows while the pointer is on the bubble and not while it is elsewhere', async () => {
    const first = bubble(0);
    expect(opacity(first.button)).toBe('0');
    await userEvent.hover(first.words);
    expect(opacity(first.button)).toBe('1');
    // Another bubble's button stays out of sight.
    expect(opacity(bubble(1).button)).toBe('0');
  });

  it('quotes the words selected in its bubble, which a press of it does not end', async () => {
    const first = bubble(0);
    window.getSelection()?.selectAllChildren(first.words);
    await userEvent.click(first.button);
    expect(quote).toHaveBeenCalledTimes(1);
    expect(quote.mock.calls[0][0].id).toBe(1);
    expect(quote.mock.calls[0][1]).toBe('Which table?');
  });

  it('quotes the whole bubble when nothing is selected in it', async () => {
    await userEvent.click(bubble(0).button);
    expect(quote).toHaveBeenCalledTimes(1);
    expect(quote.mock.calls[0][0].id).toBe(1);
    expect(quote.mock.calls[0][1]).toBeNull();
  });

  it('quotes the whole bubble when what is selected is in another bubble', async () => {
    window.getSelection()?.selectAllChildren(bubble(1).words);
    await userEvent.click(bubble(0).button);
    expect(quote).toHaveBeenCalledTimes(1);
    expect(quote.mock.calls[0][0].id).toBe(1);
    expect(quote.mock.calls[0][1]).toBeNull();
  });
});
