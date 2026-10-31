// How what the owner is asked is answered in a conversation: an ask of a work's waits pinned in the band at the top, as
// the asking agent's bubble, where it is answered, by the buttons of what it offers and a small box of the owner's
// words beside Answer, the stream keeping one line of it where it was asked; once answered it leaves the band, and the
// owner's answer is in its bubble in the stream and nowhere else; a question the architect holds is answered in its
// bubble. The questions an architect pinned wait at the bottom of its one-to-one, the newest open and the others
// folded, with no box, and the box at the bottom replies to the one open while the tick over it is on; replied to, a
// question is back where it was asked, the reply in it. And what is sent, and how the list marks each work's group and
// each architect's one-to-one by how that work or that architect stands.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { AnswersHere, answerWay, askAnswer, enterSends, handsBack, needsWords, pickAfter } from './answer';
import type {
  Architect,
  Conversation,
  ConversationDetail,
  ConversationEntry,
  DesignCard,
  GraphLines,
  Handled,
  JsonValue,
  Matter,
  MatterProgress,
  StageCard,
  OwnerAsk,
  PolicyDraft,
  Status,
  WordsState,
  WorkState,
  WorkSummary,
} from './api';
import { architectMark } from './architect';
import { course, StateMark } from './components';
import {
  asksApproval,
  DRAFT_APPROVED,
  openQuestion,
  pinnedQuestions,
  replyTicked,
  stageTitle,
  streamOf,
  toArchitect,
} from './conversation';
import { holdsWords, nowWords } from './course';
import { ordinal, time } from './format';
import { admit, IMAGE_MOST_BYTES, imagesSent, pastedFiles, type PendingImage, saysSomething } from './images';
import { Bubble } from './pages/Bubble';
import { AskView, DesignCardView } from './pages/Decide';
import {
  ArchitectPause,
  Composer,
  PauseBand,
  Pinned,
  Questions,
  type Reply,
  Stream,
  workMark,
} from './pages/Messages';
import { SayBox } from './parts';
import messagesCss from './styles/messages.css?raw';
import { t } from './text';

/// The flow's lines with their colours, as the interface serves them, which every bubble and avatar is drawn in.
const LINES: GraphLines[] = [
  {
    graph: 'engineer',
    lines: [
      { name: 'engineer', desc: 'The engineer', colour: 'blue' },
      { name: 'review', desc: 'The review', colour: 'teal' },
    ],
  },
  { graph: 'architect', lines: [{ name: 'architect', desc: 'The architect', colour: 'purple' }] },
];

// The lines are read for the whole page once; whatever else is read alike is not read yet.
vi.mock('./live', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./live')>()),
  useShared: (_key: string, path: string) => (path === '/api/v1/flows/lines' ? LINES : undefined),
}));

const ask = (id: number, body: JsonValue, answer: JsonValue | null = null): OwnerAsk => ({
  id,
  round: 2,
  work: 5,
  item: 3,
  body,
  answer,
  answer_images: [],
  asked_at: 0,
});

const entry = (source: string, extra: Partial<ConversationEntry> = {}): ConversationEntry => ({
  source,
  id: 1,
  sender: 'engineer',
  recipient: 'owner',
  text: 'said',
  at: 1,
  work: 5,
  round: 2,
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

/// The entry of an ask, as the conversation gives it: from the line that asked, to the owner, with the ask in full.
const asked = (a: OwnerAsk, extra: Partial<ConversationEntry> = {}): ConversationEntry =>
  entry('ask', { id: a.id, ask: a.id, ask_detail: a, waiting: a.answer === null, text: 'Which table?', at: 2, ...extra });

const group: Conversation = {
  key: 'work:5',
  kind: 'work',
  project: 'p',
  projects: [],
  work: 5,
  item: 3,
  title: 'Parse it',
  level: 'sonnet',
  over: false,
  waiting: 1,
  unread: 0,
  last_at: 3,
};

const detail = (entries: ConversationEntry[]): ConversationDetail => ({
  key: 'work:5',
  entries,
  asks: entries.flatMap((e) => (e.source === 'ask' && e.waiting && e.ask_detail !== null ? [e.ask_detail] : [])),
  design_cards: [],
  questions: [],
  waits_on: null,
  forwarding: null,
  read_to: null,
});

/// The stream as the page draws it in a conversation.
const drawn = (d: ConversationDetail): string =>
  renderToStaticMarkup(
    <AnswersHere.Provider value={true}>
      <Stream d={d} c={group} w={null} />
    </AnswersHere.Provider>,
  );

/// The band at the top of a conversation, what waits for the owner pinned there, as the page draws it.
const band = (d: ConversationDetail): string =>
  renderToStaticMarkup(
    <AnswersHere.Provider value={true}>
      <Pinned d={d} c={group} />
    </AnswersHere.Provider>,
  );

/// The row of a message drawn, from its mark (`data-ask="4"`, or its words) to the next row.
function row(html: string, mark: string): string {
  const at = html.indexOf(mark);
  expect(at).toBeGreaterThanOrEqual(0);
  const begin = html.lastIndexOf('<div class="talk-row', at);
  const next = html.indexOf('<div class="talk-row', at + 1);
  return html.slice(begin, next < 0 ? undefined : next);
}

/// A row's bubble, its element whole, and what comes after it in the row.
function bubble(r: string): { inside: string; after: string } {
  const start = r.indexOf('<div class="talk-bubble ');
  expect(start).toBeGreaterThanOrEqual(0);
  let depth = 0;
  for (const m of r.slice(start).matchAll(/<div\b|<\/div>/g)) {
    depth += m[0] === '</div>' ? -1 : 1;
    if (depth === 0) {
      const end = start + (m.index ?? 0) + '</div>'.length;
      return { inside: r.slice(start, end), after: r.slice(end) };
    }
  }
  return { inside: r.slice(start), after: '' };
}

/// The buttons drawn, each as its words, or a button drawn as an icon as its label (the tooltip that labels it, or its
/// own while disabled) in brackets, and whether it is pressed and disabled.
const buttons = (html: string): string[] =>
  [...html.matchAll(/<button\b([^>]*)>((?:(?!<\/button>).)*)<\/button>/g)].map((m) => {
    const by = /aria-labelledby="([^"]*)"/.exec(m[1]);
    const label = by !== null ? new RegExp(`id="${by[1]}">([^<]*)<`).exec(html) : /aria-label="([^"]*)"/.exec(m[1]);
    const text = m[2].replace(/<[^>]+>/g, '');
    const words = text !== '' || label === null ? text : `[${label[1]}]`;
    const pressed = /aria-pressed="true"/.test(m[1]) ? ' (picked)' : '';
    const disabled = /\bdisabled=""/.test(m[1]) ? ' (disabled)' : '';
    return `${words}${pressed}${disabled}`;
  });

/// The button beside every box of the owner's words that adds images, as `buttons` names it.
const ADD = `[${t('say_add_images')}]`;

const choice = {
  asked: [
    {
      kind: 'ask',
      value: {
        points: [{ kind: 'design', what: 'Which table?' }],
        choice: {
          question: 'One table or two?',
          options: [
            { name: 'One table', does: 'Keeps it simple' },
            { name: 'Two tables', does: 'Splits the reads' },
          ],
          recommended: 'One table',
        },
      },
    },
  ],
};

const words = { asked: [{ kind: 'ask', value: { points: [{ kind: 'environment', what: 'Run the probe?' }] } }] };

describe('an ask in its conversation', () => {
  it('waits pinned in the band at the top, in full, and keeps one line where it was asked that leads there', () => {
    const a = ask(4, choice);
    const d = detail([entry('message', { id: 1, text: 'Before.', at: 1 }), asked(a), entry('message', { id: 5, text: 'After.', at: 3 })]);
    expect(streamOf(d).map((e) => `${e.source}-${e.id}`)).toEqual(['message-1', 'ask-4', 'message-5']);
    // Pinned: the asking agent's bubble in full, in the engineer's colour, its bubble and its avatar alike.
    const pins = band(d);
    expect(pins).toContain('talk-pins talk-pins-top');
    const r = row(pins, 'data-ask="4"');
    expect(r).toContain('talk-bubble talk-bubble-agent role-blue');
    expect(r).toContain('talk-avatar role-blue');
    expect(r).toContain('One table or two?');
    expect(pins.match(/data-ask="4"/g)).toHaveLength(1);
    // Where it was asked: one line, who asked it to you, its question cut, and the way to the band; no card, no box.
    const html = drawn(d);
    expect(html).not.toContain('data-ask="4"');
    expect(html).not.toContain('One table or two?');
    expect(html).not.toContain('<textarea');
    const line = row(html, 'waiting for you above');
    expect(line).toMatch(/<span class="talk-who">engineer<\/span><span class="talk-mention">@you<\/span>/);
    expect(line).toContain('<span class="live-line-now">');
    expect(line).toContain('Which table?');
    expect(line).toContain('<button class="link-button live-line-link');
    expect(html.indexOf('Before.')).toBeLessThan(html.indexOf('waiting for you above'));
    expect(html.indexOf('waiting for you above')).toBeLessThan(html.indexOf('After.'));
  });

  it('is answered where it is pinned: its options as buttons, none picked, then a small box and Answer', () => {
    const { inside, after } = bubble(row(band(detail([asked(ask(4, choice))])), 'data-ask="4"'));
    // Nothing after the bubble in its row but the row's end and the band's.
    expect(after).toMatch(/^(<\/div>)+<\/section>$/);
    const answer = inside.slice(inside.indexOf('class="talk-answer"'));
    expect(buttons(answer)).toEqual(['One table', 'Two tables', ADD, 'Answer (disabled)']);
    expect(answer).toContain('<textarea');
    expect(answer).toContain('placeholder="Your answer"');
    // The ask's card in the bubble only reads: its option boxes are not picked, and it says nothing of where to answer.
    expect(inside).not.toContain('role="radio"');
    expect(inside).not.toContain('Answer it in its conversation');
  });

  it('has only the small box for an ask in words alone', () => {
    const plain = bubble(row(band(detail([asked(ask(6, words))])), 'data-ask="6"')).inside;
    expect(plain).not.toContain('talk-answer-buttons');
    expect(buttons(plain.slice(plain.indexOf('class="talk-answer"')))).toEqual([ADD, 'Answer (disabled)']);
  });

  it('once answered is one entry, one bubble, the owner’s answer in it', () => {
    const a = ask(7, words, { by: 'owner', at: 9, text: 'Good enough.' });
    const d = detail([asked(a)]);
    expect(streamOf(d)).toHaveLength(1);
    const html = drawn(d);
    expect(html.match(/class="talk-row/g)).toHaveLength(1);
    expect(html).not.toContain('talk-row-owner');
    const { inside, after } = bubble(row(html, 'data-ask="7"'));
    expect(after).toMatch(/^(<\/div>)+$/);
    expect(inside).not.toContain('<textarea');
    expect(buttons(inside)).toEqual([]);
    expect(inside).toMatch(/<span class="talk-who">Your answer to ask 7<\/span><span class="talk-when">[^<]+<\/span>/);
    expect(inside).toContain('Good enough.');
    // Its words show, with the ask in full folded under them.
    expect(inside).toContain('Ask 7 in full');
    // It has left the band.
    expect(band(d)).toBe('');
    // A long answer is folded to its first lines.
    const long = ask(7, words, { by: 'owner', at: 9, text: 'Good enough. '.repeat(100) });
    expect(drawn(detail([asked(long)]))).toContain('long long-folded');
  });

  it('closed by the engine, says in its bubble why no answer was needed', () => {
    const a = ask(4, choice, { by: 'engine', at: 9, text: 'No answer is needed: the work ended (cancelled).' });
    const { inside } = bubble(row(drawn(detail([asked(a)])), 'data-ask="4"'));
    expect(inside).toContain('No answer is needed: the work ended (cancelled).');
    expect(inside).not.toContain('Your answer');
  });
});

describe('a question the architect holds', () => {
  it('is answered in its bubble, and the owner’s answer is then in it, not an entry of its own', () => {
    const q = entry('question', { id: 8, matter: 8, recipient: 'architect', answerable: true, text: 'Split it?' });
    const open = bubble(row(drawn(detail([q])), 'Split it?')).inside;
    expect(buttons(open.slice(open.indexOf('class="talk-answer"')))).toEqual([ADD, 'Answer (disabled)']);
    const answered = { ...q, answerable: false };
    const reply = entry('reply', { id: 8, matter: 8, sender: 'owner', recipient: 'engineer', text: 'Split it in two.', at: 4 });
    const theirs = entry('reply', { id: 9, matter: 9, sender: 'architect', recipient: 'engineer', text: 'Keep it.', at: 5 });
    const d = detail([answered, reply, theirs]);
    expect(streamOf(d).map((e) => `${e.source}-${e.id}`)).toEqual(['question-8', 'reply-9']);
    const html = drawn(d);
    expect(html).not.toContain('talk-row-owner');
    const { inside } = bubble(row(html, 'Split it?'));
    expect(inside).not.toContain('<textarea');
    expect(inside).toContain('<span class="talk-who">Your answer</span>');
    expect(inside).toContain('Split it in two.');
    // The architect's reply is the architect's own message.
    expect(row(html, 'Keep it.')).toContain('talk-bubble talk-bubble-agent role-purple');
  });
});

describe('answering an ask', () => {
  it('answers by its options, or in words alone', () => {
    expect(answerWay(ask(2, choice))).toEqual({ by: 'option', options: ['One table', 'Two tables'] });
    // An open choice of the architect's is its own choice.
    const open = { from: 'architect', kind: 'open', choice: { question: 'Now?', options: [{ name: 'Yes' }, { name: 'Later' }] } };
    expect(answerWay(ask(3, open))).toEqual({ by: 'option', options: ['Yes', 'Later'] });
    expect(answerWay(ask(4, words))).toEqual({ by: 'words' });
  });

  it('picks with a click and clears with a second, and needs words only where the ask or the choice does', () => {
    expect(pickAfter('', 'One table')).toBe('One table');
    expect(pickAfter('One table', 'Two tables')).toBe('Two tables');
    expect(pickAfter('One table', 'One table')).toBe('');
    expect(needsWords('One table')).toBe(false);
    expect(needsWords('')).toBe(true);
  });

  it('sends the option picked with any words, or the words alone', () => {
    const a = ask(4, { ...choice, to: 'owner' });
    expect(handsBack(a)).toBe(true);
    expect(handsBack(ask(5, choice))).toBe(false);
    expect(askAnswer('One table', ' Keep it small. ', false)).toEqual({ text: 'Chosen option: One table. Keep it small.' });
    expect(askAnswer('One table', '', false)).toEqual({ text: 'Chosen option: One table.' });
    expect(askAnswer('', 'Neither.', true)).toEqual({ text: 'Neither.', to: 'architect' });
    expect(askAnswer('', ' ', false)).toBeNull();
    // Images may answer alone.
    expect(askAnswer('', ' ', false, true)).toEqual({ text: '' });
  });

  it('sends on Enter, and not on Shift with Enter or an Enter that ends a word of an input method', () => {
    const key = (extra: Partial<Parameters<typeof enterSends>[0]> = {}) =>
      enterSends({ key: 'Enter', shiftKey: false, keyCode: 13, nativeEvent: { isComposing: false }, ...extra });
    expect(key()).toBe(true);
    expect(key({ shiftKey: true })).toBe(false);
    expect(key({ nativeEvent: { isComposing: true } })).toBe(false);
    expect(key({ keyCode: 229 })).toBe(false);
    expect(key({ key: 'a' })).toBe(false);
  });
});

describe("a matter's bubble in the one-to-one with an architect", () => {
  const handled: Handled = {
    matter: 5,
    kind: 'set_straight',
    detail: 'missing',
    what: 'To set straight, item 12: Add the test; Note it',
    work: 9,
    round: 12,
    cell: 'H1',
    at: 3,
    reply: null,
    reply_to: null,
    told: 1,
    made: 0,
    dropped: 0,
    collected: 0,
    observations: [],
    actions: [],
    messages: [],
    ask: null,
    why: null,
    nothing: false,
  };
  const card = (extra: Partial<MatterProgress> = {}): MatterProgress => ({
    matter: 5,
    kind: 'set_straight',
    detail: 'missing',
    what: handled.what,
    state: 'done',
    cell: null,
    cell_title: null,
    architect_work: null,
    round: null,
    work: null,
    item: 12,
    handled,
    withdrawn: null,
    asked: null,
    fix: { where: 'item 12', wrong: 'Its acceptance names no test.', steps: ['Add the test.', 'Note it.'] },
    made: [],
    ...extra,
  });
  const shown = (m: MatterProgress): string =>
    drawn(detail([entry('matter', { id: m.matter, sender: 'architect', work: null, round: null, matter: m.matter, matter_card: m })]));

  it('names its kind in its head, opens its line with where it stands and what came of it, what it asks folded', () => {
    const html = shown(card());
    // The architect's bubble of one line, with no one named.
    expect(html).toContain('talk-bubble talk-bubble-agent role-purple');
    expect(html).not.toContain('talk-mention');
    // The head: who, then what it is about, its kind, in the small muted look of a kind, then when.
    expect(html).toMatch(
      /<div class="talk-bubble-head"><span class="talk-who">architect<\/span><span class="talk-card-kind">Plan fix: something missing<\/span><span class="talk-when">/,
    );
    // The line: where the matter stands as a state's chip, then what came of it; what it asks whole on hovering.
    const line = html.slice(html.indexOf('class="live-line"'), html.indexOf('</div>', html.indexOf('class="live-line"')));
    expect(line).toMatch(/data-variant="success" data-component="Label">done<\/span>/);
    expect(line).toContain('<span class="live-line-why">Told you once</span>');
    expect(line).not.toContain('Its acceptance names no test.</span>');
    expect(line).toContain('title="done | Told you once | Plan fix: something missing | item 12: Its acceptance names no test."');
    expect(html).not.toContain('To set straight');
    expect(html).not.toContain('Timeline');
    const fold = html.slice(html.indexOf('<details class="fold talk-fold"'));
    expect(fold).toMatch(/^<details class="fold talk-fold"><summary>/);
    // What it asks, whole, folded first; then its steps.
    expect(fold.indexOf('The matter in full')).toBeLessThan(fold.indexOf('The steps to fix it (2)'));
    expect(fold).toContain('<p class="talk-fold-text">item 12: Its acceptance names no test.</p>');
    expect(fold).toContain('The steps to fix it (2)');
    expect(fold).toContain('<ol');
    expect(fold).toContain('Add the test.');
    expect(fold).toContain('Note it.');
  });

  it('has no fold of steps for a matter with none, and reads working while in hand', () => {
    const html = shown(card({ kind: 'failure', detail: null, what: 'Work 4 failed its tests.', fix: null, handled: null, state: 'working' }));
    expect(html).toContain('<span class="talk-card-kind">Have a failure fixed</span>');
    expect(html).toMatch(/data-variant="accent" data-component="Label">working<\/span>/);
    expect(html).toContain('<p class="talk-fold-text">Work 4 failed its tests.</p>');
    expect([...html.matchAll(/<details/g)]).toHaveLength(1);
  });

  it('lists what it made by name, each leading to it, not folded', () => {
    const html = shown(
      card({
        fix: null,
        made: [
          { kind: 'observation', change: 'new', id: 37, title: 'Stale slots input' },
          { kind: 'item', change: 'new', id: 11, title: 'Unify test name matching' },
          { kind: 'item', change: 'reordered', id: 5, title: 'Run works in parallel' },
        ],
      }),
    );
    expect(html).toContain('Recorded observation');
    expect(html).toContain('href="/observations/37"');
    expect(html).toContain('Stale slots input');
    expect(html).toContain('New item');
    expect(html).toContain('href="/items/11"');
    expect(html).toContain('Moved in the queue');
    expect(html).toContain('href="/items/5"');
    expect(html.indexOf('Recorded observation')).toBeLessThan(html.indexOf('<details') < 0 ? html.length : html.indexOf('<details'));
  });
});

describe("the owner's words in the one-to-one with an architect", () => {
  const one: Conversation = { ...group, key: 'architect:p', kind: 'architect', work: null, item: null, title: null };
  const state = (extra: Partial<WordsState> = {}): WordsState => ({
    state: 'replying',
    project: 'p',
    architect_work: 9,
    round: 31,
    cell: 'H8',
    ...extra,
  });
  const said = (w: WordsState | null, extra: Partial<ConversationEntry> = {}): ConversationEntry =>
    entry('words', { id: 6, sender: 'owner', recipient: 'architect', text: 'hi', work: null, round: null, matter: 6, words_state: w, ...extra });
  const drawnIn = (e: ConversationEntry): string =>
    renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream d={{ ...detail([e]), key: one.key }} c={one} w={null} />
      </AnswersHere.Provider>,
    );

  it('say where they stand in one small line under their bubble, leading to the cell on the chart while it handles them', () => {
    const r = row(drawnIn(said(state())), '>hi<');
    expect(r).toMatch(/^<div class="talk-row talk-row-owner">/);
    const { after } = bubble(r);
    expect(after).toMatch(/^<p class="talk-line-more">the architect is replying \| <a [^>]*href="\/architect\?project=p&amp;round=R9-31[^"]*"[^>]*>On the chart<\/a><\/p>/);
    expect(after).toContain('cell=H8');
    expect(r).not.toContain('Your words');
    expect(r).not.toContain('not taken up yet');
  });

  it('were replied to, with no link once the architect no longer handles them', () => {
    const line = (w: WordsState) => bubble(row(drawnIn(said(w)), '>hi<')).after;
    const replied = line(state({ state: 'replied', architect_work: null, round: null, cell: null }));
    expect(replied).toMatch(/^<p class="talk-line-more">replied<\/p><\/div>/);
  });

  it('have no line while they wait to be taken up, which their head says', () => {
    const r = row(drawnIn(said(null, { pending: true })), '>hi<');
    expect(r).toContain('not taken up yet');
    expect(bubble(r).after).not.toContain('talk-line-more');
  });
});

describe("the owner's words in a work's group", () => {
  const state = (extra: Partial<WordsState> = {}): WordsState => ({
    state: 'replying',
    project: 'p',
    architect_work: 9,
    round: 31,
    cell: 'H8',
    ...extra,
  });
  const said = (w: WordsState | null): ConversationEntry =>
    entry('words', { id: 6, sender: 'owner', recipient: 'architect', text: 'Why two parsers?', matter: 6, words_state: w });

  it('say where they stand with the architect in one small line under their bubble, as in the one-to-one', () => {
    const r = row(drawn(detail([said(state())])), 'Why two parsers?');
    expect(r).toMatch(/^<div class="talk-row talk-row-owner">/);
    expect(bubble(r).after).toMatch(
      /^<p class="talk-line-more">the architect is replying \| <a [^>]*href="\/architect\?project=p&amp;round=R9-31[^"]*"[^>]*>On the chart<\/a><\/p>/,
    );
    const replied = row(drawn(detail([said(state({ state: 'replied', architect_work: null, round: null, cell: null }))])), 'Why two parsers?');
    expect(bubble(replied).after).toMatch(/^<p class="talk-line-more">replied<\/p><\/div>/);
  });
});

describe('the box of the one-to-one with an architect', () => {
  const one: Conversation = { ...group, key: 'architect:p', kind: 'architect', work: null, item: null, title: null };

  it('says something to the architect, and ends no talk', () => {
    const box = renderToStaticMarkup(<Composer c={one} w={null} />);
    expect(box).toContain('<textarea');
    expect(box).not.toContain('Done');
    expect(box).not.toContain('waits for your reply');
    expect(box).not.toContain('Reply to');
  });
});

describe('the questions an architect pinned in its one-to-one', () => {
  const one: Conversation = { ...group, key: 'architect:p', kind: 'architect', work: null, item: null, title: null };
  const question = (id: number, words: string, at: number, answer: JsonValue | null = null): ConversationEntry => {
    const a = { ...ask(id, { from: 'architect', kind: 'question', question: words, holds: [id + 10] }, answer), work: 9 };
    return asked(a, { sender: 'architect', text: words, at, work: 9, round: id });
  };
  const research = question(4, 'Research a new policy?', 2);
  const approve = question(6, 'Approve this policy? Reply exactly: approved', 3);
  const said = entry('message', { id: 1, sender: 'architect', text: 'Looking at the policy.', at: 1, work: 9, round: 1 });
  const pinning = (entries: ConversationEntry[], pinned: ConversationEntry[], waits: number | null): ConversationDetail => ({
    ...detail(entries),
    key: one.key,
    asks: [],
    questions: pinned.flatMap((e) => (e.ask_detail !== null ? [e.ask_detail] : [])),
    waits_on: waits,
  });
  const d = pinning([said, research, approve], [approve, research], 6);
  const drawnIn = (x: ConversationDetail, draft: PolicyDraft | null = null): string =>
    renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream d={x} c={one} w={null} draft={draft} />
      </AnswersHere.Provider>,
    );
  const bottom = (open: number, draft: PolicyDraft | null = null): string =>
    renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Questions pinned={pinnedQuestions(d)} open={open} onOpen={() => undefined} project="p" draft={draft} />
      </AnswersHere.Provider>,
    );

  it('wait at the bottom and not in the stream, the newest open as the architect’s message with no box, the others folded', () => {
    expect(streamOf(d).map((e) => `${e.source}-${e.id}`)).toEqual(['message-1']);
    expect(drawnIn(d)).not.toContain('data-ask');
    const pinned = pinnedQuestions(d);
    expect(pinned.map((e) => e.id)).toEqual([6, 4]);
    expect(openQuestion(pinned, null)?.id).toBe(6);
    const html = bottom(6);
    expect(html).toMatch(/^<section class="talk-pins talk-pins-bottom" aria-label="Waits for you">/);
    // In the order they were put, the newest last, over the box.
    expect(html.indexOf('Research a new policy?')).toBeLessThan(html.indexOf('data-ask="6"'));
    const open = bubble(row(html, 'data-ask="6"')).inside;
    expect(open).toContain('talk-bubble talk-bubble-agent role-purple');
    expect(open).toContain('Approve this policy? Reply exactly: approved');
    expect(open).not.toContain('<textarea');
    expect(buttons(open)).toEqual([]);
    // The older one is one line, its words whole on hovering, which a click opens.
    const folded = html.slice(html.indexOf('<button'), html.indexOf('</button>') + '</button>'.length);
    expect(folded).toMatch(/^<button type="button" class="talk-pinned-fold" title="Research a new policy\?" aria-expanded="false">/);
    expect(folded).toContain('<span class="talk-pinned-text">Research a new policy?</span>');
    expect(html.match(/data-ask=/g)).toHaveLength(1);
  });

  it('open the one the owner opened, which folds the other, and the newest again once it is no longer pinned', () => {
    const pinned = pinnedQuestions(d);
    expect(openQuestion(pinned, 4)?.id).toBe(4);
    expect(openQuestion(pinned, 99)?.id).toBe(6);
    expect(openQuestion([], null)).toBeNull();
    const html = bottom(4);
    expect(html).toContain('data-ask="4"');
    expect(html).not.toContain('data-ask="6"');
    expect(html).toContain('title="Approve this policy? Reply exactly: approved"');
  });

  it('are replied to by the box with the tick over it on, at first only while the architect waits on the one open', () => {
    const [newest, older] = pinnedQuestions(d);
    expect(replyTicked(newest, 6, null)).toBe(true);
    expect(replyTicked(older, 6, null)).toBe(false);
    expect(replyTicked(newest, null, null)).toBe(false);
    expect(replyTicked(newest, 6, { question: 6, on: false })).toBe(false);
    expect(replyTicked(older, 6, { question: 6, on: false })).toBe(false);
    expect(replyTicked(older, 6, { question: 4, on: true })).toBe(true);
    expect(replyTicked(null, 6, null)).toBe(false);
    const reply = (on: boolean): Reply => ({ question: 6, words: newest.text, on, set: () => undefined });
    const box = renderToStaticMarkup(<Composer c={one} w={null} reply={reply(true)} />);
    const tick = box.slice(0, box.indexOf('<textarea'));
    expect(tick).toMatch(/<input[^>]*type="checkbox"[^>]*checked=""/);
    expect(tick).toContain(
      '<span class="talk-reply-words" title="Approve this policy? Reply exactly: approved">Reply to: Approve this policy? Reply exactly: approved</span>',
    );
    expect(box).toContain('placeholder="Your reply to the question ticked"');
    const off = renderToStaticMarkup(<Composer c={one} w={null} reply={reply(false)} />);
    expect(off.slice(0, off.indexOf('<textarea'))).not.toMatch(/checked=""/);
    expect(off).toContain('placeholder="Say something to the architect"');
  });

  it('take the words as the reply to the question ticked, and words with no tick as words to the architect', () => {
    expect(toArchitect('approved', 6)).toEqual({ text: 'approved', reply_to: 6 });
    expect(toArchitect('Look at #3 first.', null)).toEqual({ text: 'Look at #3 first.' });
  });

  it('lead from the question whether the draft is approved, put by the round that wrote it, to the draft in the panel', () => {
    const draft: PolicyDraft = { work: 9, round: 6, at: 3, policy: { direction: 'Ship the parser.' } };
    expect(asksApproval(approve, draft)).toBe(true);
    expect(asksApproval(research, draft)).toBe(false);
    expect(asksApproval(approve, null)).toBe(false);
    const link = /<a [^>]*href="\/messages\?c=architect%3Ap&amp;side=policy"[^>]*>The draft<\/a>/;
    const pinned = bubble(row(bottom(6, draft), 'data-ask="6"')).inside;
    expect(pinned.slice(0, pinned.indexOf('talk-bubble-body'))).toMatch(link);
    expect(bottom(4, draft)).not.toMatch(link);
    // The question that asks for the approval is said to the owner, and approves with one button, which replies
    // exactly what the flow takes as the approval.
    expect(bottom(6, draft)).toContain('<span class="talk-mention">@you</span>');
    expect(buttons(bottom(6, draft))).toContain('Approve');
    expect(buttons(bottom(4, draft))).not.toContain('Approve');
    expect(DRAFT_APPROVED).toBe('approved');
    // Replied to while the draft still waits, it leads there from where it was asked.
    const replied = question(6, 'Approve this policy? Reply exactly: approved', 3, { by: 'owner', at: 5, text: 'Shorter why.' });
    expect(bubble(row(drawnIn(pinning([said, replied], [], null), draft), 'data-ask="6"')).inside).toMatch(link);
    expect(drawnIn(pinning([said, replied], [], null))).not.toMatch(link);
  });

  it('once replied to are back where they were asked, the reply in the bubble; one no longer pinned has no box', () => {
    const replied = question(4, 'Research a new policy?', 2, { by: 'owner', at: 5, text: 'Yes, research it.' });
    const gone = question(7, 'Hold #2?', 4);
    const x = pinning([said, replied, gone], [], null);
    expect(streamOf(x).map((e) => `${e.source}-${e.id}`)).toEqual(['message-1', 'ask-4', 'ask-7']);
    const html = drawnIn(x);
    const back = bubble(row(html, 'data-ask="4"')).inside;
    expect(back).toContain('Research a new policy?');
    expect(back).toMatch(/<span class="talk-who">Your answer to ask 4<\/span>/);
    expect(back).toContain('Yes, research it.');
    const left = bubble(row(html, 'data-ask="7"')).inside;
    expect(left).toContain('Hold #2?');
    expect(left).not.toContain('<textarea');
    expect(buttons(left)).toEqual([]);
    expect(row(html, 'data-ask="7"')).not.toContain('talk-row-answers');
  });
});

/// An image waiting in a box, as the box keeps it once read.
const pending = (n: number): PendingImage => ({
  key: `image-${n}`,
  name: `shot-${n}.png`,
  media_type: 'image/png',
  size: 1024,
  data_url: `data:image/png;base64,iVBOR${n}`,
});

/// A box of the owner's words drawn with images waiting in it.
const boxWith = (images: PendingImage[], text = '') =>
  renderToStaticMarkup(
    <SayBox text={text} set={() => {}} images={images} setImages={() => {}} placeholder="Say" disabled={false} onKeyDown={() => {}} />,
  );

describe('the images the owner sends with words', () => {
  it('wait over the words, each a thumbnail with a button that takes it out, with how many against the ten', () => {
    const html = boxWith([pending(1), pending(2)]);
    expect(html.indexOf('class="say-images"')).toBeLessThan(html.indexOf('<textarea'));
    expect(html.match(/<img /g)).toHaveLength(2);
    expect(html).toContain('src="data:image/png;base64,iVBOR1"');
    expect(buttons(html)).toEqual([
      `[${t('say_image_remove', { name: 'shot-1.png' })}]`,
      `[${t('say_image_remove', { name: 'shot-2.png' })}]`,
      ADD,
    ]);
    expect(html).toContain('>2 of 10<');
    // With none waiting there is no room for them; with ten, no more can be added.
    expect(boxWith([])).not.toContain('say-images');
    const full = boxWith(Array.from({ length: 10 }, (_, i) => pending(i)));
    expect(buttons(full)).toContain(`${ADD} (disabled)`);
    expect(full).toContain('>10 of 10<');
  });

  it('are taken only as PNG, JPEG, GIF or WebP images of at most 5 MB, at most ten to a message, saying why not', () => {
    const file = (name: string, type: string, size = 10) => ({ name, type, size });
    const { take, refused } = admit(8, [
      file('a.png', 'image/png'),
      file('b.pdf', 'application/pdf'),
      file('c.jpg', 'image/jpeg', IMAGE_MOST_BYTES + 1),
      file('d.webp', 'image/webp'),
      file('e.gif', 'image/gif'),
    ]);
    expect(take.map((f) => f.name)).toEqual(['a.png', 'd.webp']);
    expect(refused).toEqual([
      t('image_refused_kind', { name: 'b.pdf' }),
      t('image_refused_size', { name: 'c.jpg', size: '5.0' }),
      t('image_refused_most', { name: 'e.gif', most: 10 }),
    ]);
    expect(admit(0, [file('', 'image/png', IMAGE_MOST_BYTES)]).take).toHaveLength(1);
    expect(admit(0, [file('', 'text/plain')]).refused).toEqual([t('image_refused_kind', { name: t('image_pasted') })]);
  });

  it('are added by a paste that carries no text, and text pasted still pastes as text', () => {
    expect(pastedFiles(['Files'], ['shot'])).toEqual(['shot']);
    expect(pastedFiles(['text/html', 'Files'], ['shot'])).toEqual(['shot']);
    expect(pastedFiles(['text/plain', 'text/html', 'Files'], ['rendition'])).toEqual([]);
    expect(pastedFiles(['text/plain'], [])).toEqual([]);
  });

  it('go with the words as one message, or alone', () => {
    expect(imagesSent([pending(1), pending(2)])).toEqual([
      { media_type: 'image/png', data: 'iVBOR1' },
      { media_type: 'image/png', data: 'iVBOR2' },
    ]);
    expect(saysSomething(' ', [])).toBe(false);
    expect(saysSomething(' ', [pending(1)])).toBe(true);
    expect(saysSomething('Look.', [])).toBe(true);
  });

  it('show under the owner’s words as thumbnails, each opening at full size', () => {
    const said = entry('interjection', { id: 3, sender: 'owner', text: 'Look at these.', images: [12, 13] });
    const r = row(drawn(detail([said])), 'Look at these.');
    expect(r).toContain('href="/api/v1/attachments/12" target="_blank"');
    expect(r).toContain('src="/api/v1/attachments/13"');
    expect(r.indexOf('Look at these.')).toBeLessThan(r.indexOf('class="thumbs"'));
    // The owner's answer to an ask shows the images sent with it.
    const a = ask(7, words, { by: 'owner', at: 9, text: '' });
    const html = drawn(detail([asked({ ...a, answer_images: [21] })]));
    expect(bubble(row(html, 'data-ask="7"')).inside).toContain('src="/api/v1/attachments/21"');
  });
});

describe('the marks of the list of conversations', () => {
  const status = (extra: Partial<Status> = {}): Status => ({
    paused: false,
    keep_warm: true,
    started: [],
    releasing: [],
    waiting_for_slot: [],
    architects_started: [],
    running: null,
    stepping: null,
    last_step: null,
    slots: 1,
    slots_used: 0,
    flow_version: 2,
    config_commit: 'c',
    repos: {},
    started_at: 0,
    version: 'v',
    halt: null,
    ...extra,
  });
  const work = (id: number, extra: Partial<WorkSummary> = {}): WorkSummary => ({
    id,
    item: null,
    title: null,
    project: 'p',
    graph: 'architect',
    flow_version: 2,
    moved_from: null,
    at: 'Q1',
    at_title: 'Pick the next matter',
    stage: null,
    state: 'ready',
    paused: null,
    paused_by: null,
    ended: null,
    waits: null,
    branch: null,
    pr: null,
    started_at: 0,
    last_at: 50,
    rounds: 1,
    sent_back: 0,
    failed: 0,
    running: null,
    cost_usd: 0,
    open_asks: 0,
    open_design_cards: 0,
    ...extra,
  });
  const matter = (id: number): Matter => ({
    id,
    kind: 'owner_words',
    detail: null,
    what: `matter ${id}`,
    source: 'owner',
    item: null,
    title: null,
    work: null,
    round: null,
    at: id,
    asked: null,
    resume_cell: null,
    answered: false,
    waits: 'owner',
    waited_ms: 0,
    urgent: false,
    fix: null,
    card: null,
  });
  /// An architect with its works, newest first, and what more is given.
  const architect = (works: WorkSummary[], extra: Partial<Architect> = {}): Architect => ({
    project: 'p',
    projects: ['p'],
    works: works.map((w) => ({ work: w, handled: 0, handling: null })),
    queue: [],
    holds: [],
    asks: [],
    handled: [],
    changes: [],
    policy: null,
    draft: null,
    notebook: [],
    constitution: null,
    ...extra,
  });
  /// The Octicon a mark draws, by its name, its tone and what it says on hovering; `halo` for the running halo.
  const markOf = (html: string) => ({
    icon: html.includes('fc-halo') ? 'halo' : (/octicon-([a-z-]+)/.exec(html)?.[1] ?? null),
    tone: /state-mark-([a-z]+)/.exec(html)?.[1] ?? null,
    says: /title="([^"]*)"/.exec(html)?.[1] ?? null,
  });

  it('marks each state of a work by one Octicon in its chip’s colour, saying its chip’s words, and a round running by the halo', () => {
    const marks: Record<WorkState, { icon: string; tone: string; says: string }> = {
      running: { icon: 'halo', tone: 'accent', says: 'running' },
      ready: { icon: 'circle', tone: 'neutral', says: 'ready' },
      asking: { icon: 'question', tone: 'attention', says: 'asking' },
      waiting: { icon: 'clock', tone: 'attention', says: 'waiting' },
      waiting_for_slot: { icon: 'hourglass', tone: 'attention', says: 'waiting for a free slot' },
      paused: { icon: 'pause', tone: 'attention', says: 'paused' },
      halted: { icon: 'alert', tone: 'danger', says: 'halted' },
      done: { icon: 'check-circle', tone: 'success', says: 'done' },
      ended: { icon: 'stop', tone: 'neutral', says: 'ended' },
    };
    for (const [state, mark] of Object.entries(marks)) {
      expect(markOf(renderToStaticMarkup(<StateMark state={state as WorkState} />))).toEqual(mark);
    }
    // The halo is the chart's, its three arcs at an icon's size, and is the only mark that moves.
    const running = renderToStaticMarkup(<StateMark state="running" />);
    expect(running).toContain('<svg class="running-mark" width="16" height="16"');
    expect(running.match(/fc-halo-arc/g)).toHaveLength(3);
    // Nothing known yet: the mark's room, empty, so that the rows stay in line.
    expect(renderToStaticMarkup(<StateMark state={null} />)).toBe('<span class="state-mark" aria-hidden="true"></span>');
  });

  it('marks a work’s group by how its work stands now, and nothing before the works are read', () => {
    const works = [work(5, { graph: 'engineer', state: 'running' }), work(6, { graph: 'engineer', state: 'ended' })];
    expect(workMark(5, works, status({ running: 5 }))).toBe('running');
    // A round left open that the engine runs no more: ready, its next round closing it first.
    expect(workMark(5, works, status())).toBe('ready');
    expect(workMark(6, works, status())).toBe('ended');
    // What it waited for came, and the engine holds it for a free slot: it says so, not ready; held back by its own
    // pause, it is paused.
    const answered = [work(8, { graph: 'engineer', state: 'ready' }), work(9, { graph: 'engineer', state: 'waiting' })];
    expect(workMark(8, answered, status({ waiting_for_slot: [8, 9] }))).toBe('waiting_for_slot');
    expect(workMark(9, answered, status({ waiting_for_slot: [8, 9] }))).toBe('waiting_for_slot');
    expect(workMark(8, answered, status())).toBe('ready');
    const paused = [work(8, { graph: 'engineer', state: 'paused', paused: 'x', paused_by: 'owner' })];
    expect(workMark(8, paused, status({ waiting_for_slot: [8] }))).toBe('paused');
    expect(workMark(7, works, status())).toBeNull();
    expect(workMark(5, undefined, status())).toBeNull();
  });

  it('marks an architect as a work in the same state, with the words its head says it in', () => {
    const asked = ask(8, { from: 'architect' });
    const marked = (a: Architect, s: Status) => {
      const m = architectMark(a, s);
      return { ...m, ...markOf(renderToStaticMarkup(<StateMark state={m.state} words={m.words} />)) };
    };
    // Never run, or idle at its start: ready, saying whether matters wait for it.
    expect(marked(architect([]), status())).toMatchObject({ state: 'ready', icon: 'circle', says: 'idle: nothing in its queue' });
    const idle = architect([work(9, { state: 'done', at: 'E2' })], { queue: [matter(1)] });
    expect(architectMark(idle, status())).toEqual({ state: 'ready', words: 'takes up the matters waiting' });
    // Every work paused while matters wait for it, and the owner has not started it: paused; started, ready again.
    expect(marked(idle, status({ paused: true }))).toMatchObject({ state: 'paused', icon: 'pause', says: 'held back: every work is paused' });
    expect(architectMark(idle, status({ paused: true, architects_started: ['p'] })).state).toBe('ready');
    // A round of it running: the halo.
    const going = architect([work(10, { state: 'running' }), work(9, { state: 'done', at: 'E2' })]);
    expect(marked(going, status({ running: 10 }))).toMatchObject({ state: 'running', icon: 'halo', says: 'working' });
    // Held back while it works, by the owner's pause or every work paused: paused, as its head says why.
    const stopped = architect([work(10, { state: 'paused', paused: 'the owner paused it', paused_by: 'owner' })]);
    expect(marked(stopped, status())).toMatchObject({ state: 'paused', icon: 'pause', says: 'paused by you' });
    expect(architectMark(going, status({ paused: true })).state).toBe('paused');
    // Between rounds: ready.
    expect(architectMark(going, status())).toEqual({ state: 'ready', words: 'between rounds' });
    // A question of its own pinned for the owner's reply: asking while it neither runs nor is held back.
    expect(marked(architect([], { asks: [asked] }), status())).toMatchObject({ state: 'asking', icon: 'question', says: 'asking' });
    expect(architectMark({ ...going, asks: [asked] }, status()).state).toBe('asking');
    expect(architectMark({ ...going, asks: [asked] }, status({ running: 10 })).state).toBe('running');
    expect(architectMark({ ...stopped, asks: [asked] }, status()).state).toBe('paused');
    // Once answered, it asks no more.
    expect(architectMark(architect([], { asks: [ask(8, { from: 'architect' }, { text: 'yes' })] }), status()).state).toBe('ready');
  });
});

describe("a work's group as a chat", () => {
  const stage = (extra: Partial<StageCard> = {}): StageCard => ({
    stage: 'test',
    again: 0,
    first: 30,
    last: 41,
    started_at: 1000,
    ended_at: 61000,
    cost_usd: 3.02,
    current: false,
    outcome: 'done',
    why: null,
    findings: [],
    pr: null,
    commit: null,
    taken_over_by: null,
    notes: [
      { round: 36, line: 'engineer', text: 'Fixed F1.' },
      { round: 41, line: 'review', text: 'Six findings in the design tree.' },
    ],
    events: [],
    shown: [],
    design_cards: [],
    ...extra,
  });
  const bubble = (s: StageCard, w: WorkSummary | null = null): string =>
    renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream d={detail([entry('stage', { id: 1, at: s.started_at, round: s.first, stage: s })])} c={group} w={w} />
      </AnswersHere.Provider>,
    );
  const work = (extra: Partial<WorkSummary> = {}): WorkSummary => ({
    id: 5,
    item: 3,
    title: 'Parse it',
    project: null,
    graph: 'engineer',
    flow_version: 2,
    moved_from: null,
    at: 'M1',
    at_title: 'Judge whether it may enter the merge queue',
    stage: 'merge',
    state: 'paused',
    paused: 'Kept as it is while the trial board moves to a new build',
    paused_by: 'owner',
    ended: null,
    waits: { kind: 'ruling', design_card: 15 },
    branch: null,
    pr: null,
    started_at: 0,
    last_at: 50,
    rounds: 72,
    sent_back: 0,
    failed: 0,
    running: null,
    cost_usd: 0,
    open_asks: 0,
    open_design_cards: 1,
    ...extra,
  });

  it('names a stage by its name and which time the work is at it', () => {
    expect(stageTitle(stage())).toBe('Testing and fixing');
    expect(stageTitle(stage({ again: 1 }))).toBe('Testing and fixing, 2nd time');
    expect(stageTitle(stage({ stage: 'merge', again: 2 }))).toBe('Merge, 3rd time');
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 101].map(ordinal)).toEqual([
      '1st',
      '2nd',
      '3rd',
      '4th',
      '11th',
      '12th',
      '13th',
      '21st',
      '22nd',
      '101st',
    ]);
  });

  it("draws a stage over as the engineer's bubble of one line: how it ended, why, then the gist of its last note", () => {
    const html = bubble(
      stage({
        again: 1,
        outcome: 'sent_back',
        findings: [
          { review: 'Table review', round: 40, findings: 4 },
          { review: 'Review the design tree', round: 41, findings: 1 },
        ],
      }),
    );
    expect(html).toContain('talk-bubble talk-bubble-agent role-blue');
    // Its head: who says it, then the stage it is about in the small muted look of a kind, when, and its rounds.
    expect(html).toMatch(
      /<div class="talk-bubble-head"><span class="talk-who">engineer<\/span><span class="talk-card-kind">Testing and fixing, 2nd time<\/span><span class="talk-when">/,
    );
    expect(html).toContain('R5-30 to R5-41');
    // How long it took and what it cost, after its rounds.
    expect(html).toContain('1m 00s | $3.02');
    const line = html.slice(html.indexOf('class="live-line"'), html.indexOf('</div>', html.indexOf('class="live-line"')));
    expect(line).toContain('sent back');
    expect(line).toContain('4 findings in “Table review”; 1 finding in “Review the design tree”');
    expect(line).toContain('Six findings in the design tree.');
    expect(line).not.toContain('running-mark');
    // No cell's code, and the rest folded: what its rounds said.
    expect(html).not.toMatch(/\bT[0-9]\b|\bM1\b/);
    expect(html).toContain('What its rounds said (2)');
    // In the stream it is a bubble, not an engine's card on a timeline.
    expect(html).not.toContain('Timeline');
  });

  it('says a merge with its pull request and commit, and a work taken over with the work that took it', () => {
    const merged = bubble(stage({ stage: 'merge', outcome: 'merged', pr: 7, commit: 'abc1234def', notes: [] }));
    expect(merged).toContain('merged');
    expect(merged).toContain('PR #7');
    expect(merged).toContain('abc1234');
    expect(merged).not.toContain('abc1234def<');
    const over = bubble(
      stage({ stage: 'research', outcome: 'ended', why: 'the flow moved to version 18', taken_over_by: 31, notes: [] }),
    );
    expect(over).toContain('ended');
    expect(over).toContain('the flow moved to version 18');
    expect(over).toContain('taken over by work 31');
    expect(over).toContain('href="/messages?c=work%3A31"');
  });

  it('says what happens now on the stage the work is at, live, as the head says it, in words with no cell', () => {
    const now = stage({ stage: 'merge', first: 72, last: 72, current: true, ended_at: null, outcome: null, notes: [] });
    const html = bubble(now, work());
    expect(html).toContain('<span class="talk-who">engineer</span><span class="talk-card-kind">Merge</span>');
    expect(html).toContain('paused by you; then it waits for your ruling on design card 15 before it can merge');
    // No cell's title or code: the rounds' link leads to the cell.
    expect(html).not.toContain('Judge whether it may enter the merge queue');
    expect(html).not.toMatch(/\bM1\b/);
    expect(html).toContain('R5-72 to R5-72');
    expect(html).not.toContain('running-mark');
    // A round of the review running: who is at work, with the running mark.
    const running = work({ state: 'running', paused: null, paused_by: null, waits: null, running: { n: 73, cell: 'T4', started_at: 1, tool: false, line: 'review' } });
    const c = course(running, null);
    expect(nowWords({ ...c, running: 'goes_on' }, running)).toBe('the review is at work');
    expect(nowWords({ ...c, running: 'goes_on' }, { ...running, running: { n: 73, cell: 'X1', started_at: 1, tool: true, line: 'engineer' } })).toBe(
      "the engineer's tools are running",
    );
    // What the engine told of the work while it is at the stage shows under the line.
    expect(bubble({ ...now, events: ['Passed the limit its project sets on fixes.'] }, work())).toContain(
      'Passed the limit its project sets on fixes.',
    );
  });

  it('names every hold in the head in the order they act: the pause and why, then what it waits for once started', () => {
    const w = work();
    expect(holdsWords(course(w, null), w)).toBe(
      'Kept as it is while the trial board moves to a new build. Once started, it is waiting for your ruling on design card 15.',
    );
    // The owner's pause with no reason of the owner's says only what follows.
    const bare = work({ paused: 'the owner paused it' });
    expect(holdsWords(course(bare, null), bare)).toBe('Once started, it is waiting for your ruling on design card 15.');
    // Nothing holds it: what it waits for alone.
    const waits = work({ state: 'waiting', paused: null, paused_by: null });
    expect(holdsWords(course(waits, null), waits)).toBe('Waiting for your ruling on design card 15.');
    // The engine halted it: its own state, said as such.
    const halted = work({ state: 'halted', paused: 'cell T6: no return', paused_by: 'engine', waits: null });
    expect(nowWords(course(halted, null), halted)).toBe('halted: cell T6: no return');
  });

  it('says every live line of a stage in words, what it waits for at the merge stage what it must have to merge', () => {
    const line = (w: WorkSummary) => nowWords(course(w, null), w);
    const waiting = (waits: WorkSummary['waits'], stage = 'merge') =>
      line(work({ state: 'waiting', paused: null, paused_by: null, waits, stage }));
    expect(waiting({ kind: 'ruling', design_card: 15 })).toBe('waiting for your ruling on design card 15 before it can merge');
    expect(waiting({ kind: 'items', items: [3, 4] })).toBe('waiting for items #3, #4 to be merged before it can merge');
    // Elsewhere the stage the bubble names is where it waits.
    expect(waiting({ kind: 'answer', ask: 7 }, 'design')).toBe('waiting for your answer to ask 7');
    expect(waiting({ kind: 'architect', matter: 9 }, 'test')).toBe("waiting for the architect's reply to matter 9");
    expect(waiting({ kind: 'architect', matter: 9, project: 'q' }, 'test')).toBe(
      "waiting for the reply of q's architect to matter 9",
    );
    // A row that waits for none of these is not named by its cell and row.
    expect(waiting({ kind: 'row', cell: 'M1', row: 'wait' }, 'test')).toBe('waiting for what its next round names');
    // Held by the architect, then what it waits for.
    expect(line(work({ paused: 'Wait for #2', paused_by: 'architect', waits: { kind: 'answer', ask: 7 }, stage: 'design' }))).toBe(
      'paused by the architect; then it waits for your answer to ask 7',
    );
    // Nothing holds it and it waits for nothing: its next round is due.
    expect(line(work({ state: 'ready', paused: null, paused_by: null, waits: null }))).toBe('its next round is due');
  });

  it('names the design card pinned for the owner in one line in its stage, leading to it, and folds one ruled', () => {
    const card = (id: number, status: string): DesignCard => ({
      id,
      round: 63,
      work: 5,
      item: 3,
      status,
      body: [],
      rulings: [],
      replaced_by: null,
      replaced: null,
      cell: null,
      title: null,
      at: null,
      replaces: null,
      changed: null,
      ruled_before: null,
    });
    const s = stage({ again: 2, design_cards: [card(12, 'ruled'), card(15, 'open')] });
    const html = renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream
          d={{ ...detail([entry('stage', { id: 1, at: s.started_at, round: s.first, stage: s })]), design_cards: [card(15, 'open')] }}
          c={group}
          w={null}
        />
      </AnswersHere.Provider>,
    );
    // The one pinned is drawn once, in the band over the stream, and named here in one line that leads there.
    expect(html).not.toContain('id="design-card-15"');
    expect(html).toContain('design card 15, waiting for you above');
    expect(html).toContain('<button class="link-button');
    // One ruled stays in its stage, folded.
    expect(html).toContain('id="design-card-12"');
    expect(html).not.toContain('design card 12, waiting for you above');
    // Once it is ruled, and no longer pinned, it is folded in its stage as any other.
    const after = bubble(s);
    expect(after).toContain('id="design-card-15"');
    expect(after).not.toContain('waiting for you above');
  });

  it('says on a design card that waits that it changed since the card the owner ruled on, folded too', () => {
    const card: DesignCard = {
      id: 15,
      round: 63,
      work: 5,
      item: 3,
      status: 'open',
      body: [],
      rulings: [],
      replaced_by: null,
      replaced: null,
      cell: null,
      title: null,
      at: null,
      replaces: 14,
      changed: null,
      ruled_before: 12,
    };
    const html = renderToStaticMarkup(<DesignCardView c={card} round="R5-63" folded />);
    expect(html).toContain('changed since your ruling on design card 12: confirm it again');
    const ruled = renderToStaticMarkup(<DesignCardView c={{ ...card, status: 'ruled' }} round="R5-63" folded />);
    expect(ruled).not.toContain('changed since your ruling');
  });
});

describe('a design card and an ask, where they stand said by the chip and the facts of the head alone', () => {
  const card = (extra: Partial<DesignCard>): DesignCard => ({
    id: 15,
    round: 63,
    work: 5,
    item: 3,
    status: 'open',
    body: [
      {
        after: 'Keep one table.',
        chosen: 'One table',
        options: [{ name: 'One table' }, { name: 'Two tables' }],
      },
      { after: 'Name it runs.' },
    ],
    rulings: [],
    replaced_by: null,
    replaced: null,
    cell: null,
    title: null,
    at: null,
    replaces: null,
    changed: null,
    ruled_before: null,
    ...extra,
  });
  /// The card's head and its body, apart.
  const parts = (html: string): [string, string] => {
    const [head = '', body = ''] = html.split('class="dc-body"');
    return [head, body];
  };
  const at = new Date(2026, 9, 30, 10, 40).getTime();

  it('says in the head of a design card that waits that it blocks nothing until the work merges, with no band', () => {
    const [head, body] = parts(renderToStaticMarkup(<DesignCardView c={card({})} round="R5-63" />));
    expect(head).toContain('waits for you');
    expect(head).toContain('blocks nothing until the work merges');
    expect(body).not.toContain('Waits for your ruling');
  });

  it("marks each decision of a ruled card with its verdict and the owner's words, who ruled when in the head", () => {
    const c = card({
      status: 'ruled',
      rulings: [
        { by: 'owner', at: at - 60_000, decisions: [{ decision: 1, verdict: 'confirm' }, { decision: 2, verdict: 'confirm' }] },
        {
          by: 'owner',
          at,
          decisions: [
            { decision: 1, verdict: 'disagree', option: 'Two tables', text: 'The reads differ.' },
            { decision: 2, verdict: 'confirm' },
          ],
        },
      ],
    });
    const [head, body] = parts(renderToStaticMarkup(<DesignCardView c={c} round="R5-63" />));
    expect(head).toContain('>disagreed<');
    expect(head).toContain(`ruled by you at ${time(at)}`);
    expect(head).not.toContain('blocks nothing');
    const [first = '', second = ''] = body.split('class="dc-dec"').slice(1);
    // The verdict beside each decision, and the owner's words on the one disagreed with as what is sent to an agent,
    // the approach chosen marked among those compared.
    expect(first).toContain('>disagreed<');
    expect(first).toContain('Your ruling');
    expect(first).toMatch(/class="sheet"><p><strong>.*Two tables.*<\/strong><\/p>.*The reads differ\./);
    expect(first).toContain('your choice');
    expect(second).toContain('>confirmed<');
    expect(second).not.toContain('Your ruling');
    // The earlier ruling, folded under the decisions.
    expect(second).toMatch(/<details class="fold">.*1 earlier ruling.*Decision 1: confirm/);
    expect(second).toContain(`ruled by you at ${time(at - 60_000)}`);
  });

  it("puts a ruling made for the whole card once over the decisions, each decision marked with its verdict", () => {
    const c = card({ status: 'ruled', rulings: [{ by: 'owner', at, verdict: 'disagree', text: 'Start again.' }] });
    const [, body] = parts(renderToStaticMarkup(<DesignCardView c={c} round="R5-63" />));
    expect(body.match(/Your ruling/g)?.length).toBe(1);
    expect(body.match(/>disagreed</g)?.length).toBe(2);
    expect(body).not.toContain('earlier ruling');
  });

  it("says in the head of an ask who answered and when, and shows the answer as the owner's words", () => {
    const [waitsHead, waitsBody] = parts(renderToStaticMarkup(<AskView a={ask(4, choice)} round="R5-2" />));
    expect(waitsHead).toContain('waits for you');
    expect(waitsBody).not.toContain('Waits for your answer');
    // Its point says what it asks; an ask recorded with a kind before asks lost their kinds shows none.
    expect(waitsBody).toContain('Which table?');
    expect(waitsBody).not.toContain('>design<');
    const answered = ask(4, choice, { by: 'owner', at, choice: 'Two tables', text: 'Split them.' });
    const [head, body] = parts(renderToStaticMarkup(<AskView a={answered} round="R5-2" />));
    expect(head).toContain('>answered<');
    expect(head).toContain(`answered by you at ${time(at)}`);
    expect(body).toMatch(/Your answer.*class="sheet"><p><strong>.*Two tables.*<\/strong><\/p>.*Split them\./);
    expect(body).toContain('your choice');
    // Closed by the engine: why, in the page's own words.
    const closed = ask(4, choice, { by: 'engine', at, text: 'No answer is needed: the work ended (cancelled).' });
    const [closedHead, closedBody] = parts(renderToStaticMarkup(<AskView a={closed} round="R5-2" />));
    expect(closedHead).toContain('answered by the engine');
    expect(closedBody).toContain('<p>No answer is needed: the work ended (cancelled).</p>');
    expect(closedBody).not.toContain('class="sheet"');
  });
});

// What only the owner can do, the architect puts to the owner by pausing the work, saying why: the work's group pins
// the pause in the band at its top while it holds the work, with why and Start; a work paused otherwise pins nothing.
describe("the architect's pause of a work in its group", () => {
  const paused = (paused_by: WorkSummary['paused_by']): WorkSummary => ({
    id: 5,
    item: 3,
    title: 'Read the registry',
    project: 'p',
    graph: 'engineer',
    flow_version: 2,
    moved_from: null,
    at: 'A2',
    at_title: 'To the architect',
    stage: null,
    state: 'paused',
    paused: 'Put the registry token in the data directory',
    paused_by,
    ended: null,
    waits: null,
    branch: null,
    pr: null,
    started_at: 0,
    last_at: 50,
    rounds: 3,
    sent_back: 0,
    failed: 0,
    running: null,
    cost_usd: 0,
    open_asks: 0,
    open_design_cards: 0,
  });
  it('is pinned with why and Start while it holds the work', () => {
    const html = renderToStaticMarkup(
      <PauseBand why="Put the registry token in the data directory" onStart={() => {}} disabled={false} error={null} />,
    );
    expect(html).toContain('talk-pins talk-pins-top');
    expect(html).toContain('Put the registry token in the data directory');
    expect(html).toContain(`>${t('work_start')}<`);
    expect(renderToStaticMarkup(<ArchitectPause w={paused('owner')} />)).toBe('');
    expect(renderToStaticMarkup(<ArchitectPause w={null} />)).toBe('');
  });
});

// A bubble of one line is at most the stream's width, whatever its line holds: the stream's column is no wider than the
// stream, a row and its bubble no wider than that, and each part of the line that holds words may shrink below its
// words' length, so that the line is cut with an ellipsis at the bubble's edge, whole on hovering, and the page never
// scrolls sideways. Positions are checked in the browser; this holds the rules that keep them.
describe('a bubble of one line in a conversation', () => {
  const css = messagesCss.replace(/\/\*[\s\S]*?\*\//g, '');
  /// The declarations of the rules whose selectors include `selector`.
  const rule = (selector: string): string =>
    [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)]
      .filter((m) => m[1].split(',').map((s) => s.trim()).includes(selector))
      .map((m) => m[2])
      .join('\n');

  it('is no wider than the stream, its line cut where the bubble ends', () => {
    expect(rule('.talk-messages')).toContain('grid-template-columns: minmax(0, 1fr)');
    expect(rule('.talk-row')).toContain('max-width:');
    expect(rule('.talk-bubble')).toContain('min-width: 0');
    expect(rule('.live-line')).toContain('white-space: nowrap');
    for (const part of ['.live-line', '.live-line-state', '.live-line-now', '.live-line-why', '.live-line-gist']) {
      expect(rule(part), part).toContain('min-width: 0');
    }
    for (const part of ['.live-line-now', '.live-line-why', '.live-line-gist', '.live-made > li']) {
      expect(rule(part), part).toContain('text-overflow: ellipsis');
    }
  });
});

describe("the one-to-one with an architect as a chat", () => {
  const one: Conversation = { ...group, key: 'architect:p', kind: 'architect', work: null, item: null, title: null };
  const drawnIn = (entries: ConversationEntry[]): string =>
    renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream d={{ ...detail(entries), key: one.key }} c={one} w={null} />
      </AnswersHere.Provider>,
    );

  it("heads what the architect says to the owner with @you, as a group's message is headed", () => {
    const html = drawnIn([entry('message', { id: 1, sender: 'architect', text: 'Item 4 needs your call.', work: 9, round: 3 })]);
    expect(html).toContain('<span class="talk-mention">@you</span>');
  });

  it('says a new version of the plan in one line, the plan whole in a card that opens under it', () => {
    const plan = entry('plan', {
      id: 3,
      sender: 'architect',
      recipient: 'architect',
      text: '',
      work: 9,
      round: 12,
      plan: { changed: 'Finish #10 before #11.', plan: '1. Finish #10.\n2. Then #11.', work: 9, round: 12 },
    });
    const html = drawnIn([plan]);
    expect(html).toContain('talk-bubble talk-bubble-agent role-purple');
    expect(html).toContain('Changed the plan:');
    expect(html).toContain('Finish #10 before #11.');
    expect(html).toContain('The plan');
    expect(html).toContain('R9-12');
    expect(html).not.toContain('talk-mention');
    const first = drawnIn([{ ...plan, plan: { changed: null, plan: '1. Finish #10.', work: 9, round: 12 } }]);
    expect(first).toContain('Made the plan');
  });

  it("heads every bubble alike, who first, then to whom or what it is about, then when; only who in semibold", () => {
    const s: StageCard = {
      stage: 'merge',
      again: 2,
      first: 72,
      last: 72,
      started_at: 5,
      ended_at: null,
      cost_usd: 0,
      current: false,
      outcome: 'merged',
      why: null,
      findings: [],
      pr: null,
      commit: null,
      taken_over_by: null,
      notes: [],
      events: [],
      shown: [],
      design_cards: [],
    };
    const matter: MatterProgress = {
      matter: 5,
      kind: 'report',
      detail: 'other',
      what: 'A side report.',
      state: 'working',
      cell: null,
      cell_title: null,
      architect_work: null,
      round: null,
      work: null,
      item: null,
      handled: null,
      withdrawn: null,
      asked: null,
      fix: null,
      made: [],
    };
    const html = renderToStaticMarkup(
      <AnswersHere.Provider value={true}>
        <Stream
          d={detail([
            entry('message', { id: 1, text: 'Merged.', at: 1 }),
            entry('stage', { id: 2, at: 5, round: 72, stage: s }),
            entry('matter', { id: 3, sender: 'architect', work: null, round: null, matter: 5, matter_card: matter, at: 6 }),
            entry('plan', {
              id: 4,
              sender: 'architect',
              recipient: 'architect',
              work: 9,
              round: 12,
              at: 7,
              plan: { changed: null, plan: '1. A.', work: 9, round: 12 },
            }),
            entry('policy', {
              id: 6,
              sender: 'architect',
              work: 9,
              round: 6,
              at: 8,
              policy: { work: 9, round: 6, cell: 'P3', at: 8, policy: { direction: 'Ship the parser.' } },
            }),
          ])}
          c={group}
          w={null}
        />
      </AnswersHere.Provider>,
    );
    const heads = [...html.matchAll(/<div class="talk-bubble-head">(.*?)<\/div>/g)].map((m) => m[1]);
    expect(heads).toHaveLength(5);
    for (const h of heads) expect(h).toMatch(/^<span class="talk-who">(engineer|architect)<\/span>/);
    // To whom for what is said, what it is about for an agent's own work, in the small muted look of a kind.
    expect(heads[0]).toMatch(/^<span class="talk-who">engineer<\/span><span class="talk-mention">@you<\/span><span class="talk-when">/);
    expect(heads[1]).toMatch(/^<span class="talk-who">engineer<\/span><span class="talk-card-kind">Merge, 3rd time<\/span><span class="talk-when">/);
    expect(heads[2]).toMatch(/^<span class="talk-who">architect<\/span><span class="talk-card-kind">Go through side reports<\/span><span class="talk-when">/);
    expect(heads[3]).toMatch(/^<span class="talk-who">architect<\/span><span class="talk-when">/);
    // The state stays on the line, not in the head.
    expect(heads.join('')).not.toContain('data-component="Label"');
  });

  it('says the policy took effect in one line, leading to it', () => {
    const policy = entry('policy', {
      id: 6,
      sender: 'architect',
      text: '',
      work: 9,
      round: 6,
      policy: { work: 9, round: 6, cell: 'P3', at: 4, policy: { direction: 'Ship the parser.' } },
    });
    const html = drawnIn([policy]);
    expect(html).toContain('The new policy took effect');
    expect(html).toContain('href="/messages?c=architect%3Ap&amp;side=policy"');
    expect(html).not.toContain('Timeline');
  });
});

describe("a role's colour in a talk", () => {
  // A helper's thread with the engineer on a work's page is drawn by the same part: each helper in its own colour, on
  // the right, its avatar after its bubble, instead of one colour for every helper.
  it("draws each agent in its role's colour, a helper on the right with its avatar after its bubble", () => {
    const html = renderToStaticMarkup(
      <div className="talk-messages">
        <Bubble who="engineer" head="Engineer">
          Asked.
        </Bubble>
        <Bubble who="review" right head="Review">
          Found.
        </Bubble>
        <Bubble who="scout" head="Scout">
          Said.
        </Bubble>
      </div>,
    );
    const asked = row(html, 'Asked.');
    expect(asked).toContain('talk-avatar role-blue');
    expect(asked).toContain('talk-bubble talk-bubble-agent role-blue');
    const found = row(html, 'Found.');
    expect(found).toMatch(/^<div class="talk-row talk-row-right"/);
    expect(found).toContain('talk-bubble talk-bubble-agent role-teal');
    expect(found.indexOf('talk-avatar role-teal')).toBeGreaterThan(found.indexOf('Found.'));
    // A line the flow gives no colour is drawn in the neutral grey of an agent with no colour of its own.
    expect(row(html, 'Said.')).toContain('talk-bubble talk-bubble-agent role-none');
  });
});

