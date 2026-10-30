// How what the owner is asked is answered in a conversation: an ask shows once, where it was asked, as the asking
// agent's bubble, never pinned; it is answered in that bubble, by the buttons of what it offers and a small box of the
// owner's words beside Answer, and once answered the owner's answer is in that bubble and nowhere else; a question the
// architect holds alike. And what is sent, which asks that wait are out of view, and how the list marks each work's
// group and each architect's one-to-one by how that work or that architect stands.

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { AnswersHere, answerWay, askAnswer, enterSends, handsBack, needsWords, pickAfter } from './answer';
import type {
  Architect,
  Conversation,
  ConversationDetail,
  ConversationEntry,
  Handled,
  JsonValue,
  Matter,
  MatterProgress,
  OwnerAsk,
  Status,
  WordsState,
  WorkState,
  WorkSummary,
} from './api';
import { architectMark } from './architect';
import { StateMark } from './components';
import { awayOf, streamOf } from './conversation';
import { admit, IMAGE_MOST_BYTES, imagesSent, pastedFiles, type PendingImage, saysSomething } from './images';
import { Composer, Stream, workMark } from './pages/Messages';
import { SayBox } from './parts';
import { t } from './text';

const ask = (id: number, body: JsonValue, answer: JsonValue | null = null): OwnerAsk => ({
  id,
  round: 2,
  work: 5,
  item: 3,
  body,
  kinds: [],
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
  observation: null,
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
  it('shows once, where it was asked, as the asking agent’s bubble, and is never pinned', () => {
    const a = ask(4, choice);
    const d = detail([entry('message', { id: 1, text: 'Before.', at: 1 }), asked(a), entry('message', { id: 5, text: 'After.', at: 3 })]);
    expect(streamOf(d).map((e) => `${e.source}-${e.id}`)).toEqual(['message-1', 'ask-4', 'message-5']);
    const html = drawn(d);
    expect(html).not.toContain('talk-pins');
    expect(html.indexOf('Before.')).toBeLessThan(html.indexOf('data-ask="4"'));
    expect(html.indexOf('data-ask="4"')).toBeLessThan(html.indexOf('After.'));
    const r = row(html, 'data-ask="4"');
    expect(r).toContain('talk-bubble-engineer');
    expect(r).toContain('One table or two?');
    expect(html.match(/data-ask="4"/g)).toHaveLength(1);
  });

  it('is answered in its bubble: its options as buttons, none picked, then a small box and Answer', () => {
    const { inside, after } = bubble(row(drawn(detail([asked(ask(4, choice))])), 'data-ask="4"'));
    expect(after).toMatch(/^(<\/div>)+$/);
    const answer = inside.slice(inside.indexOf('class="talk-answer"'));
    expect(buttons(answer)).toEqual(['One table', 'Two tables', ADD, 'Answer (disabled)']);
    expect(answer).toContain('<textarea');
    expect(answer).toContain('placeholder="Your answer"');
    // The ask's card in the bubble only reads: its option boxes are not picked, and it says nothing of where to answer.
    expect(inside).not.toContain('role="radio"');
    expect(inside).not.toContain('Answer it in its conversation');
  });

  it('has only the small box for an ask in words alone', () => {
    const plain = bubble(row(drawn(detail([asked(ask(6, words))])), 'data-ask="6"')).inside;
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
    expect(row(html, 'Keep it.')).toContain('talk-bubble-architect');
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

  it('tells which way an ask that waits is out of view, and none while any of it shows', () => {
    const view = { top: 100, bottom: 500 };
    expect(awayOf({ top: 0, bottom: 90 }, view)).toBe('above');
    expect(awayOf({ top: 520, bottom: 700 }, view)).toBe('below');
    expect(awayOf({ top: 50, bottom: 120 }, view)).toBeNull();
    expect(awayOf({ top: 480, bottom: 900 }, view)).toBeNull();
    expect(awayOf({ top: 0, bottom: 900 }, view)).toBeNull();
  });
});

describe('a matter card in the one-to-one with an architect', () => {
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
    ...extra,
  });
  const shown = (m: MatterProgress): string =>
    drawn(detail([entry('matter', { id: m.matter, sender: 'architect', work: null, round: null, matter: m.matter, matter_card: m })]));

  it('says its kind and what it asks as the queue does, whole, the steps folded under it, and what became of it', () => {
    const html = shown(card());
    expect(html).toContain('Plan fix: something missing');
    expect(html).toContain('item 12: Its acceptance names no test.');
    expect(html).not.toContain('To set straight');
    const fold = html.slice(html.indexOf('<details class="fold"'));
    expect(fold).toMatch(/^<details class="fold"><summary>/);
    expect(fold).toContain('The steps to fix it (2)');
    expect(fold).toContain('<ol');
    expect(fold).toContain('Add the test.');
    expect(fold).toContain('Note it.');
    expect(html).toContain('Told you once');
  });

  it('has no fold for a matter with no steps', () => {
    const html = shown(card({ kind: 'failure', detail: null, what: 'Work 4 failed its tests.', fix: null, handled: null, state: 'working' }));
    expect(html).toContain('Have a failure fixed');
    expect(html).toContain('Work 4 failed its tests.');
    expect(html).not.toContain('<details');
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
    expect(after).toMatch(/^<p class="talk-line-more">the architect is replying \| <a [^>]*href="\/architect\?project=p&amp;work=9[^"]*"[^>]*>On the chart<\/a><\/p>/);
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

describe('the box of the one-to-one with an architect', () => {
  const one: Conversation = { ...group, key: 'architect:p', kind: 'architect', work: null, item: null, title: null };

  it('says something to the architect, and ends no talk', () => {
    const box = renderToStaticMarkup(<Composer c={one} w={null} />);
    expect(box).toContain('<textarea');
    expect(box).not.toContain('Done');
    expect(box).not.toContain('waits for your reply');
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
    works: works.map((w) => ({ work: w, started_for: [], handled: 0, handling: null })),
    queue: [],
    holds: [],
    asks: [],
    handled: [],
    changes: [],
    policy: null,
    draft: null,
    check: null,
    next: { plan: null, matters: [] },
    notebook: [],
    observations: [],
    tags: [],
    own: false,
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
      paused: { icon: 'pause', tone: 'attention', says: 'paused' },
      done: { icon: 'check-circle', tone: 'success', says: 'done' },
      ended: { icon: 'stop', tone: 'danger', says: 'ended' },
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
    expect(marked(stopped, status())).toMatchObject({ state: 'paused', icon: 'pause', says: 'stopped (you paused it)' });
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
