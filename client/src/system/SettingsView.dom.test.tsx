// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import type { AccountChoice, SettingsChange, SettingsPage, SystemSource } from './api';
import { SystemSourceContext } from './api';
import { fixtureAccounts, fixtureSettings, fixtureSystem } from './fixtures';
import { SettingsView } from './SettingsView';

let unmounts: (() => void)[] = [];
afterEach(() => {
  for (const u of unmounts) u();
  unmounts = [];
  document.body.innerHTML = '';
  localStorage.clear();
});

interface SettingsSource {
  sent: SettingsChange[];
  chosen: AccountChoice[];
  page: SettingsPage;
  refuse: string | null;
  reads: number;
  source: SystemSource;
}

/// The settings' fixtures, with what is sent kept, a save answered as the test says, and the file read anew after it.
function settingsSource(page: SettingsPage = fixtureSettings()): SettingsSource {
  const sent: SettingsChange[] = [];
  const chosen: AccountChoice[] = [];
  const s: SettingsSource = {
    sent,
    chosen,
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
      chooseAccount: async (choice: AccountChoice) => {
        chosen.push(choice);
        return { said: 'agents.config_dir = "/data/agents/claude-second" instead of "/data/agents/claude"', committed: true };
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
    type(box(page, 'agents.slots'), '40');
    expect(save(page).disabled).toBe(true);
    expect(page.querySelector('ul.failed')?.textContent).toBe('agents.slots is a whole number from 1 to 32');
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

  it('says why a save was refused, and keeps what was typed through a reload', async () => {
    const s = settingsSource();
    s.refuse = 'the settings would not read: agents.slots: at least one round must be able to run';
    const page = draw(s.source);
    await settle();
    type(box(page, 'owner.language'), 'English');
    act(() => save(page).click());
    await settle();
    expect(page.querySelector('p.failed')?.textContent).toBe(`Refused: ${s.refuse}`);
    const again = draw(s.source);
    await settle();
    expect(box(again, 'owner.language').value).toBe('English');
  });

  it('drops what was typed against the file before it changed, and says so', async () => {
    localStorage.setItem('settings-draft', JSON.stringify({ base: 'older', values: { 'agents.slots': '9' } }));
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    expect(box(page, 'agents.slots').value).toBe('3');
    expect(page.textContent).toContain('was typed on the file as it was before it changed, and was dropped');
  });

  it('saves nothing onto changes not committed, and says the file does not read', async () => {
    const s = settingsSource({ ...fixtureSettings(), git: { committed: false }, unreadable: 'items.unknown: no such key' });
    const page = draw(s.source);
    await settle();
    type(box(page, 'agents.slots'), '4');
    expect(save(page).disabled).toBe(true);
    expect(save(page).title).toContain('has changes not committed');
    expect(page.querySelector('.read-failed')?.textContent).toBe('The file does not read as settings: items.unknown: no such key');
  });

  it('switches the account the agents run under once the owner says yes beside it', async () => {
    const s = settingsSource();
    const page = draw(s.source);
    await settle();
    const rows = [...page.querySelectorAll('table[aria-label="The accounts"] tbody tr')];
    expect(rows.map((r) => r.firstElementChild?.textContent)).toEqual(['claude', 'claude-second', 'claude-third']);
    expect(rows[0].textContent).toContain('named');
    expect(rows[2].querySelector('button')).toBeNull();
    act(() => (rows[1].querySelector('button') as HTMLButtonElement).click());
    expect(rows[1].textContent).toContain('The agents run under second@example.com once the engine starts again');
    const useIt = [...rows[1].querySelectorAll('button')].find((b) => b.textContent === 'Use it') as HTMLButtonElement;
    act(() => useIt.click());
    await settle();
    expect(s.chosen).toEqual([{ base: 'fixture', name: 'claude-second' }]);
    const name = page.querySelector('#account-name') as HTMLInputElement;
    type(name, 'fourth');
    expect(page.querySelector('code.command')?.textContent).toBe('CLAUDE_CONFIG_DIR=/data/agents/claude-fourth claude');
  });
});
