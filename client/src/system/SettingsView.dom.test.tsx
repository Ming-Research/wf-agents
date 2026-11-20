// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DEFAULT_ACCENT, ownName, pickAccent, PRESETS, startAccent, type Theme, TONES, tones } from '../accent';
import { type Source, SourceContext, sourceOf } from '../works/source';
import type { Changed } from '../works-api';
import type { AccountAsked, AccountDone, AccountsPage, SettingsChange, SettingsPage, SystemSource } from './api';
import { SystemSourceContext } from './api';
import { fixtureAccounts, fixtureSettings, fixtureSystem } from './fixtures';
import { SettingsView } from './SettingsView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
  pickAccent(null);
  localStorage.clear();
  document.documentElement.removeAttribute('style');
  startAccent();
});

interface SettingsSource {
  sent: SettingsChange[];
  asked: AccountAsked[];
  page: SettingsPage;
  accounts: AccountsPage;
  refuse: string | null;
  /// How an act on an account is answered: at once, as done or refused, or held until the test answers it.
  answer: (asked: AccountAsked) => Promise<AccountDone>;
  reads: number;
  /// How many times the accounts were read, and looked under.
  accountReads: number;
  looks: number;
  source: SystemSource;
  /// The works' source, whose stream of changes the test says what happened down ([`SettingsSource.changed`]).
  works: Source;
  changed: (changed: Changed) => void;
}

const DONE: AccountDone = { atLimit: false, resets: null, notLetGo: null };

/// The settings' fixtures, with what is sent kept, a save answered as the test says, and the file read anew after it.
function settingsSource(page: SettingsPage = fixtureSettings()): SettingsSource {
  const sent: SettingsChange[] = [];
  const asked: AccountAsked[] = [];
  const listening = new Set<(changed: Changed) => void>();
  const s: SettingsSource = {
    sent,
    asked,
    page,
    accounts: fixtureAccounts(),
    answer: async () => DONE,
    refuse: null,
    reads: 0,
    accountReads: 0,
    looks: 0,
    works: {
      ...sourceOf(''),
      changes: (onChanged) => {
        listening.add(onChanged);
        return () => listening.delete(onChanged);
      },
    },
    changed: (changed) => listening.forEach((on) => on(changed)),
    source: {
      loadSystem: async (weeks: number[]) => fixtureSystem(weeks),
      loadSettings: async (): Promise<SettingsPage> => {
        s.reads += 1;
        return s.page;
      },
      saveSettings: async (change: SettingsChange) => {
        sent.push(change);
        if (s.refuse !== null) throw new Error(s.refuse);
        return { said: 'agents.slots = 4 instead of 3', committed: true };
      },
      loadAccounts: async () => {
        s.accountReads += 1;
        return s.accounts;
      },
      account: async (act: AccountAsked) => {
        asked.push(act);
        return s.answer(act);
      },
      look: async () => {
        s.looks += 1;
        return { looking: [] };
      },
    },
  };
  return s;
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function draw(source: SystemSource, works?: Source): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const quiet: Source = { ...sourceOf(''), changes: () => () => undefined };
  act(() =>
    root.render(
      <SourceContext.Provider value={works ?? quiet}>
        <SystemSourceContext.Provider value={source}>
          <SettingsView />
        </SystemSourceContext.Provider>
      </SourceContext.Provider>,
    ),
  );
  unmounts.push(() => act(() => root.unmount()));
  return container;
}

/// Types into a box as the owner does.
function type(box: HTMLInputElement | HTMLSelectElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(box), 'value')?.set;
  act(() => {
    setter?.call(box, value);
    box.dispatchEvent(new Event(box instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }));
  });
}

const box = (root: ParentNode, key: string) => root.querySelector(`[aria-label="${key}"]`) as HTMLInputElement;
const accountRows = (root: ParentNode) => [...root.querySelectorAll('table[aria-label="The accounts"] tbody tr')];
const buttons = (root: ParentNode) => [...root.querySelectorAll('button')].map((b) => b.textContent);
const button = (root: ParentNode, text: string) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent === text) as HTMLButtonElement;
const accountBox = (root: ParentNode, what: 'name' | 'token') =>
  root.querySelector(`#account-${what}`) as HTMLInputElement;
/// Everything this browser keeps, as one text.
const storedText = () =>
  Object.keys(localStorage)
    .map((key) => `${key}=${localStorage.getItem(key)}`)
    .join('\n');
const save = (root: ParentNode) =>
  [...root.querySelectorAll('.view-line button')].find((b) => b.textContent === 'Save') as HTMLButtonElement;

describe('the Settings view', () => {
  it('shows each key the page changes in a box, and the rest of the file as it writes it, with what the engine runs with', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    expect(page.querySelector('.view-facts')?.textContent).toBe('/data/install.toml');
    expect(box(page, 'agents.slots').value).toBe('3');
    expect(box(page, 'owner.language').value).toBe('Chinese');
    expect(box(page, 'items.highest_level').value).toBe('3');
    const slots = box(page, 'agents.slots').closest('tr')!;
    expect(slots.lastElementChild?.textContent).toBe('2');
    const limit = box(page, 'sessions.context_limit').closest('tr')!;
    expect(limit.lastElementChild?.textContent).toBe('500000 · left out of the file');
    const fileOnly = [...page.querySelectorAll('.file-table')];
    expect(fileOnly.map((t) => t.querySelector('h4')?.textContent)).toEqual([
      '[project]',
      '[agents]',
      '[agents.levels.1]',
      '[agents.levels.2]',
      '[agents.levels.3]',
      '[items]',
      '[github]',
    ]);
    const agents = fileOnly[1];
    expect([...agents.querySelectorAll('dt')].map((d) => d.textContent)).toEqual([
      'command',
      'args',
      'work_budget_usd',
    ]);
    expect(page.textContent).toContain('"WF_GITHUB_TOKEN"');
    expect(save(page).disabled).toBe(true);
    expect(save(page).title).toBe('Nothing is changed yet');
  });

  it('saves only what changed, a count as a number, and says what was done', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    type(box(page, 'agents.slots'), 'four');
    expect(save(page).disabled).toBe(true);
    expect(page.querySelector('ul.failed')?.textContent).toBe('agents.slots is a whole number');
    type(box(page, 'agents.slots'), '4');
    expect(save(page).disabled).toBe(false);
    act(() => save(page).click());
    await settle();
    expect(s.sent).toEqual([{ base: 'fixture', set: [{ key: 'agents.slots', value: 4 }] }]);
    expect(page.querySelector('.said')?.textContent).toBe(
      'Saved: agents.slots = 4 instead of 3, and committed. The engine takes it as it starts again.',
    );
    expect(s.reads).toBe(2);
  });

  it('checks only the keys changed, and leaves to the engine what only it refuses', async () => {
    const page0 = fixtureSettings();
    const agents = page0.tables.find((t) => t.name === 'agents')!;
    agents.keys = agents.keys.map((k) => (k.key === 'slots' ? { ...k, written: '0' } : k));
    const s = settingsSource(page0);
    const page = draw(s.source);
    await settle();
    type(box(page, 'owner.language'), 'English');
    expect(save(page).disabled).toBe(false);
    expect(page.querySelector('ul.failed')).toBeNull();
  });

  it('keeps what was typed through a refusal, the file changed under it, and a reload, and drops a key the file now has', async () => {
    const s = settingsSource();
    s.refuse = 'install.toml changed since the page read it: read it again, and change it then';
    const page = draw(s.source);
    await settle();
    type(box(page, 'owner.language'), 'English');
    type(box(page, 'agents.slots'), '5');
    // Someone changed the file: the save is refused, the file read anew, and what was typed stays on it.
    const changed = fixtureSettings();
    changed.base = 'changed';
    const items = changed.tables.find((t) => t.name === 'items')!;
    items.keys = items.keys.map((k) => (k.key === 'default_level' ? { ...k, written: '"1"' } : k));
    s.page = changed;
    act(() => save(page).click());
    await settle();
    expect(page.querySelector('p.failed')?.textContent).toBe(`Refused: ${s.refuse}`);
    expect(box(page, 'owner.language').value).toBe('English');
    expect(box(page, 'agents.slots').value).toBe('5');
    s.refuse = null;
    act(() => save(page).click());
    await settle();
    expect(s.sent.at(-1)).toEqual({
      base: 'changed',
      set: [
        { key: 'owner.language', value: 'English' },
        { key: 'agents.slots', value: 5 },
      ],
    });
    // Typed again and the page drawn anew, as on a reload: what was typed comes back, but a key whose value the file
    // now has is no longer a change.
    type(box(page, 'owner.language'), 'French');
    type(box(page, 'agents.slots'), '6');
    const later = fixtureSettings();
    later.base = 'later';
    const agents = later.tables.find((t) => t.name === 'agents')!;
    agents.keys = agents.keys.map((k) => (k.key === 'slots' ? { ...k, written: '6' } : k));
    s.page = later;
    const again = draw(s.source);
    await settle();
    expect(box(again, 'owner.language').value).toBe('French');
    expect(box(again, 'agents.slots').value).toBe('6');
    expect(JSON.parse(localStorage.getItem('settings-draft') ?? '{}')).toEqual({ 'owner.language': 'French' });
  });

  it('saves nothing onto changes not committed, and says the file does not read', async () => {
    const s = settingsSource({ ...fixtureSettings(), git: { committed: false }, unreadable: 'items.unknown: no such key' });
    const page = draw(s.source);
    await settle();
    type(box(page, 'agents.slots'), '4');
    expect(save(page).disabled).toBe(true);
    expect(save(page).title).toContain('has changes not committed');
    expect(page.querySelector('.read-failed')).toBeNull();
    expect(page.querySelector('.view-fault')?.textContent).toBe('The file does not read as settings: items.unknown: no such key');
  });

  it('lists the accounts: the one in use marked, the usage of each, and a token ending within a month', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    const rows = accountRows(page);
    expect(page.querySelector('table[aria-label="The accounts"] thead')?.textContent).toContain('Usage');
    expect(rows.map((r) => r.firstElementChild?.textContent)).toEqual(['work', 'home', 'old']);
    expect(rows[0].querySelector('.account-in-use')?.textContent).toBe('in use');
    expect(rows[0].querySelector('button')).toBeNull();
    expect(buttons(rows[1])).toEqual(['Use', 'Remove']);
    // Each window the service named, by its own name, with when it was said.
    const work = [...rows[0].children[3].children];
    expect(work.map((d) => d.textContent?.replace(/resets .*$/, 'resets …'))).toEqual([
      'five_hour 20% · resets …',
      'seven_day 67% · resets …',
      expect.stringMatching(/^seen /),
    ]);
    expect(work[0].className).toBe('');
    // The engine's limit, and a window the service no longer lets through, in the look of a warning.
    const home = [...rows[1].children[3].children];
    expect(home[0].textContent).toMatch(/^at its limit · resets /);
    expect(home[0].className).toBe('warn-text');
    expect(home[1].textContent).toMatch(/^seven_day 100% · resets /);
    expect(home[1].className).toBe('warn-text');
    expect(rows[2].children[3].textContent).toBe('not seen');
    // The token added 350 days ago ends within a month: its row says so, in the look of a warning.
    expect(rows[2].children[2].className).toBe('warn-text');
    expect(rows[2].children[2].textContent).toContain(' · ends in 15 days');
    expect(rows[0].children[2].className).toBe('');
    expect(page.textContent).not.toContain('No account is');
    expect(page.querySelector('code.command')?.textContent).toBe('claude setup-token');
  });

  it('looks under the accounts as it opens, says a look under way and one that failed, and reads them again on a change', async () => {
    const s = settingsSource();
    const accounts = fixtureAccounts();
    const failed = 'The network does not reach the agents\' service.';
    const looking = { seen: null, windows: [], looking: true, failed: null };
    accounts.accounts[1] = { ...accounts.accounts[1], limit: null, usage: looking };
    accounts.accounts[2] = { ...accounts.accounts[2], usage: { ...looking, looking: false, failed } };
    s.accounts = accounts;
    const page = draw(s.source, s.works);
    await settle();
    expect(s.looks).toBe(1);
    expect(accountRows(page)[1].children[3].textContent).toBe('looking');
    expect(accountRows(page)[2].children[3].querySelector('.failed')?.textContent).toBe(failed);
    // The looks come back: the engine says the accounts changed, and the view reads them again; a change of the works
    // alone reads nothing.
    const reads = s.accountReads;
    s.accounts = fixtureAccounts();
    act(() => s.changed({ works: [1], items: false, copies: false, accounts: false }));
    await settle();
    expect(s.accountReads).toBe(reads);
    act(() => s.changed({ works: [], items: false, copies: false, accounts: true }));
    await settle();
    expect(s.accountReads).toBe(reads + 1);
    expect(accountRows(page)[1].children[3].textContent).toContain('seven_day 100%');
    expect(s.looks, 'looked once, as it opened').toBe(1);
  });

  it('takes an account at once with Use, and asks once beside an account before removing it', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    const use = button(accountRows(page)[1], 'Use');
    expect(use.title).toBe(
      "Run the agents under home from now on: each line's next round writes its session to home's cache",
    );
    act(() => use.click());
    await settle();
    expect(s.asked).toEqual([{ kind: 'take', name: 'home' }]);
    expect(page.querySelector('.said')?.textContent).toBe('The agents run under home.');
    act(() => button(accountRows(page)[2], 'Remove').click());
    expect(accountRows(page)[2].querySelector('.ask-line')?.textContent).toContain('Remove old and its token?');
    act(() => button(accountRows(page)[2], 'Cancel').click());
    expect(accountRows(page)[2].querySelector('.ask-line')).toBeNull();
    expect(s.asked).toHaveLength(1);
    act(() => button(accountRows(page)[2], 'Remove').click());
    act(() => button(accountRows(page)[2], 'Remove it').click());
    await settle();
    expect(s.asked[s.asked.length - 1]).toEqual({ kind: 'remove', name: 'old' });
    expect(page.querySelector('.said')?.textContent).toBe('Removed old.');
  });

  it('adds an account: says it is checking the token, then what came of it, and keeps the name typed but never the token', async () => {
    const s = settingsSource();
    const held: { answer?: (done: AccountDone) => void } = {};
    s.answer = () => new Promise((resolve) => (held.answer = resolve));
    const page = draw(s.source);
    await settle();
    const add = () => button(page.querySelector('.account-add')!, 'Add');
    expect(add().disabled).toBe(true);
    type(accountBox(page, 'name'), 'max');
    expect(add().title).toBe('Paste the token the command printed');
    type(accountBox(page, 'token'), 'sk-ant-oat01-secret');
    expect(accountBox(page, 'token').type).toBe('password');
    expect(storedText()).toContain('max');
    expect(storedText()).not.toContain('sk-ant');
    act(() => add().click());
    expect(s.asked).toEqual([{ kind: 'add', name: 'max', token: 'sk-ant-oat01-secret' }]);
    expect(page.querySelector('.account-add ~ [role="status"]')?.textContent).toBe(
      'Checking the token: this may take up to two minutes.',
    );
    expect(add().disabled).toBe(true);
    await act(async () => held.answer?.({ atLimit: true, resets: Date.UTC(2026, 10, 19, 15), notLetGo: null }));
    await settle();
    expect(page.querySelector('.said')?.textContent).toMatch(
      /^Added max: its token is at its usage limit, which resets /,
    );
    expect(accountBox(page, 'token').value).toBe('');
    expect(accountBox(page, 'name').value).toBe('');
    expect(storedText()).not.toContain('sk-ant');
  });

  it('says the engine\'s words where a token is refused or could not be checked, and a name that is no name or is taken', async () => {
    const s = settingsSource();
    s.answer = async () => {
      throw new Error('No account was added: the service refused the token: Invalid bearer token.');
    };
    const page = draw(s.source);
    await settle();
    const add = () => button(page.querySelector('.account-add')!, 'Add');
    type(accountBox(page, 'name'), 'Max!');
    type(accountBox(page, 'token'), 'sk-ant-oat01-secret');
    expect(add().title).toBe('A name is small letters, digits and dashes, beginning with a letter or a digit');
    type(accountBox(page, 'name'), 'home');
    expect(add().title).toBe('An account named home is added already');
    type(accountBox(page, 'name'), 'max');
    act(() => add().click());
    await settle();
    expect(page.querySelector('.account-add ~ .failed')?.textContent).toBe(
      'Refused: No account was added: the service refused the token: Invalid bearer token.',
    );
    s.answer = async () => {
      throw new Error(
        'No account was added: the token could not be checked, so it was not kept: offline; add it again.',
      );
    };
    act(() => add().click());
    await settle();
    expect(page.querySelector('.account-add ~ [role="alert"]')?.textContent).toContain('could not be checked');
    expect(storedText()).not.toContain('sk-ant');
  });

  it('says in one line that no round runs while no account is in use, and why none can be added where none can', async () => {
    const s = settingsSource();
    const missing = '/usr/bin/security is not on this system, and no token can be kept in the login keychain';
    s.accounts = { ...fixtureAccounts(), accounts: [], inUse: null, closed: missing };
    const page = draw(s.source);
    await settle();
    expect(accountRows(page)).toEqual([]);
    expect(page.textContent).toContain("No account is in use, and no agent's round runs until one is.");
    expect(page.textContent).not.toContain('sign-in');
    type(accountBox(page, 'name'), 'max');
    type(accountBox(page, 'token'), 'sk-ant-oat01-secret');
    const add = button(page.querySelector('.account-add')!, 'Add');
    expect(add.disabled).toBe(true);
    expect(add.title).toBe(missing);
    expect(page.textContent).toContain(`No account can be added or taken, and no agent runs: ${missing}`);
  });

  it("says in the look of a failure that a removed account's token is still kept, and shows no limit past its reset", async () => {
    const s = settingsSource();
    const accounts = fixtureAccounts();
    const home = accounts.accounts[1];
    accounts.accounts[1] = { ...home, limit: { since: accounts.now - 7_200_000, resets: accounts.now - 1 } };
    s.accounts = accounts;
    s.answer = async () => ({ atLimit: false, resets: null, notLetGo: 'old: User interaction is not allowed.' });
    const page = draw(s.source);
    await settle();
    expect(accountRows(page)[1].children[3].textContent).not.toContain('at its limit');
    expect(accountRows(page)[1].children[3].textContent).toContain('seven_day 100%');
    act(() => button(accountRows(page)[2], 'Remove').click());
    act(() => button(accountRows(page)[2], 'Remove it').click());
    await settle();
    const said = page.querySelector('.account-add ~ [role="alert"]');
    expect(said?.className).toBe('failed');
    expect(said?.textContent).toBe(
      'Removed old, but a token is still in the keychain, and is let go of again at the next removal and as the ' +
        'engine starts: old: User interaction is not allowed.',
    );
  });
});

/// The tones the root has of the accent, each by its name, or none where it has none set.
function onRoot(): Record<string, string> {
  const style = document.documentElement.style;
  return Object.fromEntries(
    (['light', 'dark'] as Theme[]).flatMap((theme) =>
      Object.keys(TONES[theme])
        .map((tone) => [ownName(theme, tone), style.getPropertyValue(ownName(theme, tone))])
        .filter(([, v]) => v !== ''),
    ),
  );
}

/// The tones a colour gives, as the root is to have them.
function given(colour: string): Record<string, string> {
  const t = tones(colour);
  return Object.fromEntries(
    (['light', 'dark'] as Theme[]).flatMap((theme) =>
      Object.keys(TONES[theme]).map((tone) => [ownName(theme, tone), t[theme][tone]]),
    ),
  );
}

const choices = (root: ParentNode) =>
  [...root.querySelectorAll('[aria-label="The accent, kept in this browser"] button')] as HTMLButtonElement[];
const choice = (root: ParentNode, name: string) =>
  choices(root).find((b) => b.textContent === name) as HTMLButtonElement;
const pressed = (root: ParentNode) =>
  choices(root)
    .filter((b) => b.getAttribute('aria-pressed') === 'true')
    .map((b) => b.textContent);
const ownBox = (root: ParentNode) => root.querySelector('input.accent-box') as HTMLInputElement;
const alerts = (root: ParentNode) =>
  [...root.querySelectorAll('.accent-pick ~ [role="alert"]')].map((a) => a.textContent?.replace(/\s+/g, ' '));

/// Picks a colour in the browser's picker: each step of a drag sends `input`, and closing the picker `change`.
function pickOwn(root: ParentNode, colour: string, close = true) {
  const box = ownBox(root);
  act(() => {
    box.value = colour;
    box.dispatchEvent(new Event('input', { bubbles: true }));
    if (close) box.dispatchEvent(new Event('change', { bubbles: true }));
  });
}

describe("the Settings view's accent", () => {
  it('stands last once the file is read or could not be, in the default accent at first, with nothing set on the root', async () => {
    const page = draw(settingsSource().source);
    await settle();
    const parts = [...page.querySelectorAll('.panel-part-label')].map((h) => h.textContent);
    expect(parts.at(-1)).toBe('Accent, kept in this browser');
    expect(choices(page).map((b) => b.textContent)).toEqual(['Blue', 'Indigo', 'Violet', 'Purple']);
    expect(pressed(page)).toEqual(['Blue']);
    expect(ownBox(page).value).toBe(DEFAULT_ACCENT);
    expect(onRoot()).toEqual({});
    // Each preset's mark is the accent it gives, in light and in dark.
    const marks = choices(page).map((b) => b.querySelector('.accent-mark') as HTMLElement);
    expect(
      marks.map((m) => [m.style.getPropertyValue('--accent-mark'), m.style.getPropertyValue('--accent-mark-dark')]),
    ).toEqual(PRESETS.map((p) => [tones(p.colour).light.accent, tones(p.colour).dark.accent]));

    // While the file is read, the part waits with the rest, so that it does not stand first and then move.
    const reading = settingsSource();
    reading.source.loadSettings = () => new Promise<SettingsPage>(() => undefined);
    const waiting = draw(reading.source);
    await settle();
    expect(waiting.querySelector('.message')?.textContent).toBe('Reading the settings');
    expect(waiting.querySelector('.accent-pick')).toBeNull();

    const s = settingsSource();
    s.source.loadSettings = async () => {
      throw new Error('the engine is not running');
    };
    const unread = draw(s.source);
    await settle();
    expect(unread.querySelector('.message')?.textContent).toBe(
      'The page could not read the settings: the engine is not running',
    );
    expect(pressed(unread)).toEqual(['Blue']);
  });

  it('draws the page in a preset as it is pressed, outside what Save writes, keeps it, and has it again as the page starts anew', async () => {
    const page = draw(settingsSource().source);
    await settle();
    const violet = PRESETS[2];
    act(() => choice(page, 'Violet').click());
    expect(onRoot()).toEqual(given(violet.colour));
    expect(onRoot()['--own-accent']).toBe(violet.colour);
    expect(pressed(page)).toEqual(['Violet']);
    expect(ownBox(page).value).toBe(violet.colour);
    expect(localStorage.getItem('wf-agents.accent')).toBe(violet.colour);
    expect(save(page).disabled).toBe(true);
    expect(save(page).title).toBe('Nothing is changed yet');

    // The page started anew: the root as the browser loads it, the accent read from what this browser keeps.
    for (const u of unmounts) u();
    unmounts = [];
    document.documentElement.removeAttribute('style');
    startAccent();
    expect(onRoot()).toEqual(given(violet.colour));
    const again = draw(settingsSource().source);
    await settle();
    expect(pressed(again)).toEqual(['Violet']);

    act(() => choice(again, 'Blue').click());
    expect(onRoot()).toEqual({});
    expect(localStorage.getItem('wf-agents.accent')).toBeNull();
    expect(pressed(again)).toEqual(['Blue']);
  });

  it("takes a colour of the owner's own as the picker closes, and leaves one near a meaning untaken, saying why once", async () => {
    const page = draw(settingsSource().source);
    await settle();
    // A step of a drag in the picker takes nothing yet.
    pickOwn(page, '#1e3a8a', false);
    expect(onRoot()).toEqual({});
    pickOwn(page, '#1e3a8a');
    expect(onRoot()).toEqual(given('#1e3a8a'));
    expect(pressed(page)).toEqual([]);
    expect(page.querySelector('.accent-own')?.classList.contains('accent-own-taken')).toBe(true);
    expect(localStorage.getItem('wf-agents.accent')).toBe('#1e3a8a');
    expect(alerts(page)).toEqual([]);

    pickOwn(page, '#ec4899');
    expect(onRoot()).toEqual(given('#1e3a8a'));
    expect(localStorage.getItem('wf-agents.accent')).toBe('#1e3a8a');
    expect(ownBox(page).value).toBe('#1e3a8a');
    expect(alerts(page)).toEqual([
      '#ec4899 is too near the red of a problem, which keeps its meaning: the accent stays as it was, and takes a colour from blue to purple.',
    ]);
    pickOwn(page, '#808080');
    expect(alerts(page)).toEqual([
      '#808080 is too grey to stand apart from the muted words and a pause: the accent stays as it was, and takes a colour from blue to purple.',
    ]);

    act(() => choice(page, 'Indigo').click());
    expect(alerts(page)).toEqual([]);
    expect(pressed(page)).toEqual(['Indigo']);
    expect(page.querySelector('.accent-own')?.classList.contains('accent-own-taken')).toBe(false);
    // The default accent picked as one's own is the default: nothing set, nothing kept.
    pickOwn(page, DEFAULT_ACCENT.toUpperCase());
    expect(onRoot()).toEqual({});
    expect(pressed(page)).toEqual(['Blue']);
    expect(localStorage.getItem('wf-agents.accent')).toBeNull();
  });

  it('takes a kept value that is not a colour as none, one in capitals as itself, and says of a kept colour refused now why the page is in its default accent', async () => {
    localStorage.setItem('wf-agents.accent', PRESETS[1].colour.toUpperCase());
    startAccent();
    expect(onRoot()).toEqual(given(PRESETS[1].colour));
    localStorage.setItem('wf-agents.accent', 'blue');
    startAccent();
    expect(onRoot()).toEqual({});
    const page = draw(settingsSource().source);
    await settle();
    expect(pressed(page)).toEqual(['Blue']);
    expect(alerts(page)).toEqual([]);
    for (const u of unmounts) u();
    unmounts = [];

    localStorage.setItem('wf-agents.accent', '#e5484d');
    startAccent();
    expect(onRoot()).toEqual({});
    const refused = draw(settingsSource().source);
    await settle();
    expect(pressed(refused)).toEqual(['Blue']);
    expect(alerts(refused)).toEqual([
      'The colour kept in this browser is refused now: #e5484d is too near the red of a problem, which keeps its meaning. The page is drawn in its own blue until another is picked.',
    ]);
    act(() => choice(refused, 'Purple').click());
    expect(alerts(refused)).toEqual([]);
  });

  it('draws the page in a colour picked where the browser keeps nothing, and throws nothing', async () => {
    const refuse = () => {
      throw new Error('the browser keeps nothing here');
    };
    vi.stubGlobal('localStorage', {
      getItem: refuse,
      setItem: refuse,
      removeItem: refuse,
      clear: refuse,
      key: refuse,
      length: 0,
    });
    startAccent();
    const page = draw(settingsSource().source);
    await settle();
    act(() => choice(page, 'Purple').click());
    expect(onRoot()).toEqual(given(PRESETS[3].colour));
    expect(pressed(page)).toEqual(['Purple']);
    vi.unstubAllGlobals();
    expect(localStorage.getItem('wf-agents.accent')).toBeNull();
  });
});
