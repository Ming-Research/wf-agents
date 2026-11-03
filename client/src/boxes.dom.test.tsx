// @vitest-environment happy-dom
// What the owner types into each box of the page is still there when the box is left and shown again, and when the page
// is loaded again, counted in a DOM: the box at the bottom of a conversation, the small boxes in a bubble that answer an
// ask and an engineer's question, the box on a decision card, the reason for closing an observation, the settings form,
// the name of an account to add and the ref of the flow box's comparison. Each is typed in as the owner does, taken
// away, drawn again, and drawn again after the store forgot what it held in memory (a page loaded again); a box is never
// another's; a send that is on its way greys the box shown again, and a send that came through leaves it empty.

import './dom-setup';

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CliAccounts, Conversation, DesignCard, FlowVersion, Observation, OwnerAsk, Settings as SettingsData } from './api';
import { forgetMemory } from './drafts';
import { CompareBox } from './pages/Flow';
import { AskAnswer, Composer, QuestionAnswer } from './pages/Messages';
import { Close } from './pages/Observations';
import { CardRule } from './pages/Decide';
import { AddAccount, Form } from './pages/Settings';
import { t } from './text';

const WORDS_AT = 'wf-agents.draft.';

let root: Root | null = null;
let host: HTMLElement | null = null;

/// Draws `ui` in the document, inside `act`.
async function show(ui: ReactNode): Promise<HTMLElement> {
  host = document.createElement('div');
  document.body.append(host);
  const mine = createRoot(host);
  root = mine;
  await act(async () => mine.render(ui));
  return host;
}

/// Takes what is drawn away, as leaving the page or the conversation does.
async function leave(): Promise<void> {
  const mine = root;
  if (mine !== null) await act(async () => mine.unmount());
  host?.remove();
  root = null;
  host = null;
}

type Box = HTMLTextAreaElement | HTMLInputElement;

/// Types `value` into `box` as the owner does: the browser's own setter, then the event React listens to.
async function type(box: Box, value: string): Promise<void> {
  const proto = box instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const set = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  await act(async () => {
    set?.call(box, value);
    box.dispatchEvent(new Event('input', { bubbles: true }));
  });
}

/// A click on `el`.
const click = (el: Element) => act(async () => (el as HTMLElement).click());

/// Lets the requests under way end and what they changed be drawn.
const settle = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });

/// The element `find` finds in what is drawn, which a test cannot go on without.
function must<T>(found: T | null | undefined): T {
  if (found === null || found === undefined) throw new Error('not drawn');
  return found;
}

const textarea = (h: HTMLElement): Box => must(h.querySelector('textarea'));
const firstInput = (h: HTMLElement): Box => must(h.querySelector('input'));

/// The button of `h` named `name`.
const button = (h: HTMLElement, name: string): HTMLButtonElement =>
  must([...h.querySelectorAll('button')].find((b) => b.textContent?.trim() === name));

/// What the browser keeps of the words of the draft under `key`.
const keptWords = (key: string): string | undefined => {
  const raw = localStorage.getItem(WORDS_AT + key);
  return raw === null ? undefined : (JSON.parse(raw) as { text: string }).text;
};

/// A storage that holds what it is given in memory: the tests run on one of their own, since Node may have a
/// `localStorage` of its own that the DOM does not replace.
function storage(): Storage {
  const m = new Map<string, string>();
  return {
    get length() {
      return m.size;
    },
    key: (i: number) => [...m.keys()][i] ?? null,
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
    removeItem: (k: string) => void m.delete(k),
    clear: () => m.clear(),
  };
}

/// What the fake interface answers: a POST waits for `release` when `hold` is set, then comes through or is refused.
const posts: Array<{ path: string; body: unknown }> = [];
let hold: Promise<void> | null = null;
let refuse: string | null = null;

beforeEach(() => {
  vi.stubGlobal('localStorage', storage());
  forgetMemory();
  posts.length = 0;
  hold = null;
  refuse = null;
  vi.stubGlobal('fetch', async (path: string, init?: { method?: string; body?: string }) => {
    if (init?.method === 'POST') {
      posts.push({ path, body: JSON.parse(init.body ?? 'null') });
      await hold;
      if (refuse !== null) {
        return { ok: false, status: 500, statusText: 'Server Error', json: async () => ({ error: refuse }) };
      }
      return { ok: true, text: async () => '' };
    }
    if (path === '/api/v1/flows/sources') {
      return { ok: true, text: async () => JSON.stringify([{ project: 'p', checkout: '/c', refs: ['main'] }]) };
    }
    return { ok: false, status: 404, statusText: 'Not Found', json: async () => ({ error: 'not found' }) };
  });
});

afterEach(async () => {
  await leave();
  vi.unstubAllGlobals();
});

/// Draws `ui`, types `text` into the box `pick` finds, takes `ui` away and draws it again, and again after the store
/// forgot what it held in memory: the box shows `text` each time, and the browser keeps it under `key`.
async function keeps(ui: ReactNode, pick: (h: HTMLElement) => Box, key: string, text = 'a long reply\nin two lines') {
  let h = await show(ui);
  await type(pick(h), text);
  expect(pick(h).value).toBe(text);
  expect(keptWords(key)).toBe(text);
  await leave();
  h = await show(ui);
  expect(pick(h).value).toBe(text);
  await leave();
  forgetMemory();
  h = await show(ui);
  expect(pick(h).value).toBe(text);
}

const work = (n: number): Conversation => ({
  key: `work:${n}`,
  kind: 'work',
  project: 'p',
  projects: [],
  work: n,
  item: null,
  title: null,
  level: null,
  over: false,
  waiting: 0,
  unread: 0,
  last_at: 1,
});

const ask = (id: number): OwnerAsk => ({
  id,
  round: 1,
  work: 5,
  item: null,
  body: { question: 'Which table?' },
  answer: null,
  answer_images: [],
  asked_at: 1,
});

const card = (id: number): DesignCard => ({ id, work: 5 }) as DesignCard;
const observation = (id: number): Observation => ({ id }) as Observation;

describe('the box at the bottom of a conversation', () => {
  it("keeps the words and whom they are for, and is not another conversation's", async () => {
    await keeps(<Composer c={work(5)} w={null} />, textarea, 'conv:work:5');
    await leave();
    // Whom the words are for: a work's group says to the engineer at first; the architect ticked too is kept.
    let h = await show(<Composer c={work(5)} w={null} />);
    const ticks = () => [...h.querySelectorAll('input[type="checkbox"]')].map((x) => (x as HTMLInputElement).checked);
    expect(ticks()).toEqual([true, false]);
    await click(must(h.querySelectorAll('input[type="checkbox"]')[1]));
    expect(ticks()).toEqual([true, true]);
    await leave();
    forgetMemory();
    h = await show(<Composer c={work(5)} w={null} />);
    expect(ticks()).toEqual([true, true]);
    await leave();
    // Another conversation's box has nothing of it.
    h = await show(<Composer c={work(6)} w={null} />);
    expect(textarea(h).value).toBe('');
    expect(ticks()).toEqual([true, false]);
    // And a conversation whose number begins with this one's is another.
    await leave();
    h = await show(<Composer c={work(50)} w={null} />);
    expect(textarea(h).value).toBe('');
  });

  it('is greyed when shown again while its words are on their way, and empty when shown after they came through', async () => {
    let release!: () => void;
    hold = new Promise<void>((r) => (release = r));
    let h = await show(<Composer c={work(5)} w={null} />);
    await type(textarea(h), 'on its way');
    await click(button(h, t('conv_send')));
    expect(posts).toHaveLength(1);
    expect(textarea(h).disabled).toBe(true);
    await leave();
    h = await show(<Composer c={work(5)} w={null} />);
    expect(textarea(h).disabled).toBe(true);
    expect(textarea(h).value).toBe('on its way');
    // Away again while it comes through: nothing brings the words back.
    await leave();
    release();
    await settle();
    h = await show(<Composer c={work(5)} w={null} />);
    expect(textarea(h).value).toBe('');
    expect(textarea(h).disabled).toBe(false);
    expect(keptWords('conv:work:5')).toBeUndefined();
  });

  it('keeps the words and says why when the send is refused, also when shown again', async () => {
    refuse = 'the engine said no';
    let h = await show(<Composer c={work(5)} w={null} />);
    await type(textarea(h), 'my words');
    await click(button(h, t('conv_send')));
    await settle();
    expect(textarea(h).value).toBe('my words');
    expect(h.textContent).toContain('the engine said no');
    await leave();
    h = await show(<Composer c={work(5)} w={null} />);
    expect(textarea(h).value).toBe('my words');
    expect(textarea(h).disabled).toBe(false);
    expect(h.textContent).toContain('the engine said no');
  });
});

describe('the small boxes in a bubble', () => {
  it("keep what answers an ask, and an ask's box is not another ask's", async () => {
    await keeps(<AskAnswer a={ask(17)} />, textarea, 'ask:17');
    await leave();
    let h = await show(<AskAnswer a={ask(18)} />);
    expect(textarea(h).value).toBe('');
    await leave();
    // 1 is the beginning of 17, and no more.
    h = await show(<AskAnswer a={ask(1)} />);
    expect(textarea(h).value).toBe('');
  });

  it("keep what answers an engineer's question, and a question's box is not another's", async () => {
    await keeps(<QuestionAnswer work={5} matter={9} />, textarea, 'question:5:9');
    await leave();
    let h = await show(<QuestionAnswer work={5} matter={90} />);
    expect(textarea(h).value).toBe('');
    await leave();
    h = await show(<QuestionAnswer work={50} matter={9} />);
    expect(textarea(h).value).toBe('');
  });
});

describe('the box on a decision card', () => {
  it("keeps the owner's words, and a card's box is not another card's", async () => {
    await keeps(<CardRule c={card(42)} picked="" recommended="a" />, textarea, 'card:42');
    await leave();
    const h = await show(<CardRule c={card(43)} picked="" recommended="a" />);
    expect(textarea(h).value).toBe('');
  });
});

describe('the reason for closing an observation', () => {
  it("is kept, and an observation's reason is not another's", async () => {
    await keeps(<Close o={observation(9)} />, textarea, 'obs-close:9', 'fixed in #31');
    await leave();
    const h = await show(<Close o={observation(10)} />);
    expect(textarea(h).value).toBe('');
  });
});

describe('the name of an account to add', () => {
  const claude: CliAccounts = { cli: 'claude', command: 'claude', dir: '/d', env: 'CLAUDE_CONFIG_DIR', login: '/login', accounts: [] };

  it("is kept, and one CLI's name is not another's", async () => {
    await keeps(<AddAccount c={claude} onDone={() => undefined} />, firstInput, 'account:claude', 'work');
    await leave();
    const h = await show(<AddAccount c={{ ...claude, cli: 'codex' }} onDone={() => undefined} />);
    expect(firstInput(h).value).toBe('');
  });
});

describe("the ref of the flow's comparison", () => {
  const versions: FlowVersion[] = [
    { id: 1, digest: 'aaaaaaaa', works: 0 },
    { id: 2, digest: 'bbbbbbbb', works: 1 },
  ];
  const route = { page: 'flow', version: null, graph: null, cell: null, project: null, ref: null, snapshot: null, from: null } as const;
  const compare = <CompareBox r={route} versions={versions} shown={2} current={2} onClose={() => undefined} />;
  const refBox = (h: HTMLElement): Box => must(h.querySelector<HTMLInputElement>('input[list="flow-refs"]'));
  /// Chooses to compare with a ref, which enables the box for it.
  const chooseRef = async (h: HTMLElement) => click(must(h.querySelector('input[type="radio"][value="ref"]')));

  it('is kept while the box is closed and opened again', async () => {
    let h = await show(compare);
    await chooseRef(h);
    await type(refBox(h), 'release/2');
    expect(keptWords('flow-compare-ref')).toBe('release/2');
    await leave();
    h = await show(compare);
    await chooseRef(h);
    expect(refBox(h).value).toBe('release/2');
    await leave();
    forgetMemory();
    h = await show(compare);
    await chooseRef(h);
    expect(refBox(h).value).toBe('release/2');
  });

  it('shows the ref the flow shown is compared at until the owner types, and starts again from it after Compare', async () => {
    const at = { ...route, project: 'p', ref: 'main', from: 1 } as const;
    let h = await show(<CompareBox r={at} versions={versions} shown={null} current={2} onClose={() => undefined} />);
    await settle();
    expect(refBox(h).value).toBe('main');
    // Cleared by the owner, it stays cleared: it does not fill itself with the fallback again.
    await type(refBox(h), '');
    expect(refBox(h).value).toBe('');
    await type(refBox(h), 'topic');
    await settle();
    // Compare with a project chosen (it is read from the interface) uses the ref and ends the draft.
    await click(button(h, t('compare_show')));
    expect(keptWords('flow-compare-ref')).toBeUndefined();
    await leave();
    h = await show(<CompareBox r={at} versions={versions} shown={null} current={2} onClose={() => undefined} />);
    expect(refBox(h).value).toBe('main');
  });
});

describe('the settings form', () => {
  const settings: SettingsData = {
    tiers: [
      { name: '1', model: 'claude-haiku-5-5', desc: 'small', keep_warm_most: 19 },
      { name: '2', model: 'claude-sonnet-5-5', desc: 'known', keep_warm_most: 39 },
    ],
    highest_level: '2',
    architect: '2',
    context_limit: 500_000,
    keep_warm_minutes: 55,
    language: 'Chinese',
    hosts: [{ name: 'macbook', slots: 2 }],
    clis: [{ name: 'claude', command: 'claude', args: ['--tools', 'Bash,Read'] }],
    gh_allow: ['pr view'],
    query_max_rows: 200,
    query_timeout_ms: 5000,
    projects: [
      {
        name: 'wf-agents',
        limits: [{ counter: 'asks', limit: 10, desc: 'questions' }],
        observation_tags: [{ name: 'flow', desc: 'The flow graphs' }],
        flow_feedback: true,
        repo: 'o/wf-agents',
        checkout: '/w',
        main: 'main',
        test_paths: ['tests/'],
        records: null,
        ci_fast: 'ci',
        ci_full: ['ci'],
        merge_checks: ['check'],
        constitution: 'CONSTITUTION.md',
        rules: [],
      },
    ],
    rules: {},
    commit: 'abcdef0123',
  };
  const form = (s: SettingsData) => <Form s={s} said={null} onSaid={() => undefined} />;
  /// The box for the language, the first of the form.
  const language = (h: HTMLElement): Box => firstInput(h);

  it('keeps the unsaved changes when the commit is the same, and not when the files changed', async () => {
    let h = await show(form(settings));
    await type(language(h), 'Japanese');
    await leave();
    h = await show(form(settings));
    expect(language(h).value).toBe('Japanese');
    await leave();
    forgetMemory();
    h = await show(form(settings));
    expect(language(h).value).toBe('Japanese');
    expect(h.textContent).not.toContain(t('settings_draft_dropped'));
    await leave();
    // The files changed: the disk's values are shown, with a word about it, and the changes are gone for good.
    forgetMemory();
    const later = { ...settings, commit: 'bbbbbbbbbb', language: 'Korean' };
    h = await show(form(later));
    expect(language(h).value).toBe('Korean');
    expect(h.textContent).toContain(t('settings_draft_dropped'));
    expect(keptWords('settings')).toBeUndefined();
    await leave();
    h = await show(form(later));
    expect(language(h).value).toBe('Korean');
    expect(h.textContent).not.toContain(t('settings_draft_dropped'));
  });

  it('does not keep a form that is put back to the settings as they are, and drops words it cannot read', async () => {
    let h = await show(form(settings));
    await type(language(h), 'Japanese');
    expect(keptWords('settings')).toBeDefined();
    await type(language(h), 'Chinese');
    expect(keptWords('settings')).toBeUndefined();
    // Words that are no form of these settings are not shown as one, and not saved.
    localStorage.setItem(WORDS_AT + 'settings', JSON.stringify({ text: '{"language":7}', at: Date.now(), base: 'abcdef0123' }));
    await leave();
    forgetMemory();
    h = await show(form(settings));
    expect(language(h).value).toBe('Chinese');
  });

  it('starts again without a word about it from the commit its own save made', async () => {
    let h = await show(form(settings));
    await type(language(h), 'Japanese');
    await click(button(h, t('settings_save')));
    await settle();
    expect(posts).toHaveLength(1);
    expect(posts[0].body).toMatchObject({ language: 'Japanese' });
    // Until the commit it made is read, the form goes on showing what was saved, and keeps no draft of it.
    expect(language(h).value).toBe('Japanese');
    expect(keptWords('settings')).toBeUndefined();
    await leave();
    h = await show(form({ ...settings, commit: 'cccccccccc', language: 'Japanese' }));
    expect(language(h).value).toBe('Japanese');
    expect(h.textContent).not.toContain(t('settings_draft_dropped'));
  });
});
