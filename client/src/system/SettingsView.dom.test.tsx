// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ownName, pickAccent, PRESETS, startAccent, type Theme, TONES, tones, VIOLET } from '../accent';
import type { AccountsChange, Saved, SettingsChange, SettingsPage, SystemSource } from './api';
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
  changed: AccountsChange[];
  /// Why the next change of the accounts is refused; none where it is taken.
  refuseAccounts: string | null;
  page: SettingsPage;
  refuse: string | null;
  reads: number;
  source: SystemSource;
}

/// The settings' fixtures, with what is sent kept, a save answered as the test says, and the file read anew after it.
function settingsSource(page: SettingsPage = fixtureSettings()): SettingsSource {
  const sent: SettingsChange[] = [];
  const changed: AccountsChange[] = [];
  const s: SettingsSource = {
    sent,
    changed,
    refuseAccounts: null,
    page,
    refuse: null,
    reads: 0,
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
      loadAccounts: async () => fixtureAccounts(),
      changeAccounts: async (change: AccountsChange): Promise<Saved | null> => {
        changed.push(change);
        if (s.refuseAccounts !== null) throw new Error(s.refuseAccounts);
        return 'add' in change ? { said: 'agents.accounts = […]', committed: true } : null;
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

function draw(source: SystemSource): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <SystemSourceContext.Provider value={source}>
        <SettingsView />
      </SystemSourceContext.Provider>,
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
      'config_dir',
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

  it('shows the accounts in order, takes one in a click apart from what Save writes, sets the switch, and adds a new one', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    type(box(page, 'agents.slots'), '4');
    const heads = [...page.querySelectorAll('.panel-part-label')].map((h) => h.textContent);
    expect(heads.slice(0, 3)).toEqual([
      'Changed here',
      'Accounts the agents run under (agents.accounts)',
      'Changed in install.toml only',
    ]);
    const rows = [...page.querySelectorAll('table[aria-label="The accounts"] tbody tr')];
    const cells = rows.map((r) => [...r.children].map((c) => c.textContent));
    expect(cells.map((c) => c[0])).toEqual(['claude', 'claude-second', 'claude-third', 'claude-fourth']);
    expect(cells[0][2]).toMatch(/^at its limit until \d\d-\d\d \d\d:\d\d$/);
    expect(cells[1][3]).toBe('in use');
    expect(cells[2][3]).toBe('sign it in first');
    expect(cells[3][3]).toBe('taken as the engine starts again');
    expect(page.textContent).toContain("each line's next round begins a new session under it, which reads its context again");
    // One click takes an account, and nothing typed for Save is lost or sent.
    const use = rows[0].querySelector('button') as HTMLButtonElement;
    expect(use.textContent).toBe('Use');
    act(() => use.click());
    expect(use.disabled).toBe(true);
    await settle();
    expect(s.changed).toEqual([{ take: { path: '/data/agents/claude' } }]);
    expect(s.sent).toEqual([]);
    expect(box(page, 'agents.slots').value).toBe('4');
    // The switch is off until set, and set in a click.
    const moving = [...page.querySelectorAll('[aria-label="Move on at the week\'s limit"] button')] as HTMLButtonElement[];
    expect(moving.map((b) => [b.textContent, b.getAttribute('aria-pressed')])).toEqual([
      ['Off', 'true'],
      ['On', 'false'],
    ]);
    act(() => moving[1].click());
    await settle();
    expect(s.changed.at(-1)).toEqual({ movingOn: { on: true } });
    // The accounts taken, the latest first: the engine's says which it left and until when.
    const taken = [...page.querySelectorAll('.account-taken li')].map((li) => li.textContent);
    expect(taken).toHaveLength(2);
    expect(taken[0]).toMatch(/ claude-second by the engine: claude at its limit until \d\d-\d\d \d\d:\d\d$/);
    expect(taken[1]).toMatch(/ claude by you$/);
    // A new account: the command that signs it in, then Add, refused where it is not signed in yet.
    type(page.querySelector('#account-name') as HTMLInputElement, 'fifth');
    expect(page.querySelector('code.command')?.textContent).toBe('CLAUDE_CONFIG_DIR=/data/agents/claude-fifth claude');
    s.refuseAccounts = 'claude-fifth is not signed in: run `CLAUDE_CONFIG_DIR=/data/agents/claude-fifth claude` and type /login there first';
    const add = [...page.querySelectorAll('.account-add button')].find((b) => b.textContent === 'Add') as HTMLButtonElement;
    act(() => add.click());
    await settle();
    expect(s.changed.at(-1)).toEqual({ add: { base: 'fixture', name: 'fifth' } });
    expect(page.querySelector('.account-add ~ p.failed')?.textContent).toBe(`Refused: ${s.refuseAccounts}`);
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
const choice = (root: ParentNode, name: string) => choices(root).find((b) => b.textContent === name) as HTMLButtonElement;
const pressed = (root: ParentNode) => choices(root).filter((b) => b.getAttribute('aria-pressed') === 'true').map((b) => b.textContent);
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
  it("stands last once the file is read or could not be, in the page's own violet at first, with nothing set on the root", async () => {
    const page = draw(settingsSource().source);
    await settle();
    const parts = [...page.querySelectorAll('.panel-part-label')].map((h) => h.textContent);
    expect(parts.at(-1)).toBe('Accent, kept in this browser');
    expect(choices(page).map((b) => b.textContent)).toEqual(['Blue', 'Indigo', 'Violet', 'Purple']);
    expect(pressed(page)).toEqual(['Violet']);
    expect(ownBox(page).value).toBe(VIOLET);
    expect(onRoot()).toEqual({});
    // Each preset's mark is the accent it gives, in light and in dark.
    const marks = choices(page).map((b) => b.querySelector('.accent-mark') as HTMLElement);
    expect(marks.map((m) => [m.style.getPropertyValue('--accent-mark'), m.style.getPropertyValue('--accent-mark-dark')])).toEqual(
      PRESETS.map((p) => [tones(p.colour).light.accent, tones(p.colour).dark.accent]),
    );

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
    expect(unread.querySelector('.message')?.textContent).toBe('The page could not read the settings: the engine is not running');
    expect(pressed(unread)).toEqual(['Violet']);
  });

  it('draws the page in a preset as it is pressed, outside what Save writes, keeps it, and has it again as the page starts anew', async () => {
    const page = draw(settingsSource().source);
    await settle();
    const blue = PRESETS[0];
    act(() => choice(page, 'Blue').click());
    expect(onRoot()).toEqual(given(blue.colour));
    expect(onRoot()['--own-accent']).toBe(blue.colour);
    expect(pressed(page)).toEqual(['Blue']);
    expect(ownBox(page).value).toBe(blue.colour);
    expect(localStorage.getItem('wf-agents.accent')).toBe(blue.colour);
    expect(save(page).disabled).toBe(true);
    expect(save(page).title).toBe('Nothing is changed yet');

    // The page started anew: the root as the browser loads it, the accent read from what this browser keeps.
    for (const u of unmounts) u();
    unmounts = [];
    document.documentElement.removeAttribute('style');
    startAccent();
    expect(onRoot()).toEqual(given(blue.colour));
    const again = draw(settingsSource().source);
    await settle();
    expect(pressed(again)).toEqual(['Blue']);

    act(() => choice(again, 'Violet').click());
    expect(onRoot()).toEqual({});
    expect(localStorage.getItem('wf-agents.accent')).toBeNull();
    expect(pressed(again)).toEqual(['Violet']);
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
    // The violet picked as one's own is the page's own: nothing set, nothing kept.
    pickOwn(page, VIOLET.toUpperCase());
    expect(onRoot()).toEqual({});
    expect(pressed(page)).toEqual(['Violet']);
    expect(localStorage.getItem('wf-agents.accent')).toBeNull();
  });

  it('takes a kept value that is not a colour as none, one in capitals as itself, and says of a kept colour refused now why the page is in its violet', async () => {
    localStorage.setItem('wf-agents.accent', PRESETS[1].colour.toUpperCase());
    startAccent();
    expect(onRoot()).toEqual(given(PRESETS[1].colour));
    localStorage.setItem('wf-agents.accent', 'blue');
    startAccent();
    expect(onRoot()).toEqual({});
    const page = draw(settingsSource().source);
    await settle();
    expect(pressed(page)).toEqual(['Violet']);
    expect(alerts(page)).toEqual([]);
    for (const u of unmounts) u();
    unmounts = [];

    localStorage.setItem('wf-agents.accent', '#e5484d');
    startAccent();
    expect(onRoot()).toEqual({});
    const refused = draw(settingsSource().source);
    await settle();
    expect(pressed(refused)).toEqual(['Violet']);
    expect(alerts(refused)).toEqual([
      'The colour kept in this browser is refused now: #e5484d is too near the red of a problem, which keeps its meaning. The page is drawn in its own violet until another is picked.',
    ]);
    act(() => choice(refused, 'Purple').click());
    expect(alerts(refused)).toEqual([]);
  });

  it('draws the page in a colour picked where the browser keeps nothing, and throws nothing', async () => {
    const refuse = () => {
      throw new Error('the browser keeps nothing here');
    };
    vi.stubGlobal('localStorage', { getItem: refuse, setItem: refuse, removeItem: refuse, clear: refuse, key: refuse, length: 0 });
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
