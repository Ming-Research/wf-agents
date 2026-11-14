// A console's card that waits for the owner, in a real browser (Chromium): it is tinted in the orange that asks for the
// owner, as the console on `old-main` tinted it, with no glow, and says on a third row where its work waits: the cell
// by its id and its title, and what it waits for as keys and values. A card the owner paused is tinted grey, apart from
// a question, and says it is paused; one the engine stopped keeps its fill and its glow.

import '../styles/tokens.css';
import '../styles/parts.css';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import type { Now } from '../works-api';
import type { Shown, Where } from './items';
import { type DrawnCard, SnapScene } from './snap';

/// An item whose work does `now`, waiting for the owner where `where` says.
const card = (id: number, now: Now, where: Where | null, asks = 0): Shown => ({
  id,
  title: `Item ${id}`,
  standing: 'open',
  asks,
  questions: 0,
  deliver: false,
  blocked: 0,
  unseen: false,
  unread: 0,
  begun: true,
  shadow: false,
  work: {
    number: id,
    now,
    paused: now.kind === 'paused',
    bar: '',
    sentBack: 0,
    cells: [{ cell: 'R2', state: 'done', finished: 1, record: 1, open: null }],
    from: Date.now(),
    usdMicros: 0,
    last: null,
    where,
  },
  children: [],
});

/// A token of the page as `#rrggbb`.
const token = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

/// The colour a part `k` of the way from `a` to `b`, both `#rrggbb`.
const mixed = (a: string, b: string, k: number) => {
  const at = (h: string, i: number) => Number.parseInt(h.slice(1 + 2 * i, 3 + 2 * i), 16);
  const part = (i: number) => Math.round(at(a, i) + (at(b, i) - at(a, i)) * k);
  return `#${[0, 1, 2].map((i) => part(i).toString(16).padStart(2, '0')).join('')}`;
};

describe("a card that waits for the owner", () => {
  let host: HTMLDivElement;
  let scene: SnapScene;
  beforeEach(async () => {
    host = document.createElement('div');
    host.style.cssText = 'position: fixed; left: 0; top: 0; width: 1000px; height: 640px';
    document.body.append(host);
    scene = new SnapScene();
    await scene.mount(host);
  });
  afterEach(() => {
    scene.destroy();
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  const asked: Where = { cell: 'R2', title: 'Is there a better way', says: [['answer', 'scope']] };
  const paused: Where = { cell: 'M1', title: 'Check before the merge', says: [['paused', null]] };

  for (const theme of ['light', 'dark']) {
    it(`is tinted in the owner's orange with no glow, and says where it waits, in ${theme}`, async () => {
      document.documentElement.setAttribute('data-theme', theme);
      // The scene takes the theme's colours as it hears of the change.
      await new Promise((done) => setTimeout(done, 0));
      scene.show([
        card(1, { kind: 'waits', at: 'R2', on: [{ kind: 'answer', question: 'scope' }] }, asked, 1),
        card(2, { kind: 'paused', by: 'owner', why: null }, paused),
        card(3, { kind: 'paused', by: 'engine', why: 'A round failed.' }, null),
      ]);
      const [owner, held, halted] = [1, 2, 3].map((id) => scene.drawnCard(id) as DrawnCard);
      const ground = token('--card');
      expect(owner.glows).toBe(false);
      expect(owner.fill).toBe(mixed(ground, token('--orange'), 0.1));
      expect(owner.edge).toBe(mixed(ground, token('--orange'), 0.45));
      expect(owner.where?.words).toEqual(['R2', 'Is there a better way', 'answer', 'scope']);

      // Paused by the owner: tinted, but not in the owner's orange, and says so where a question would stand.
      expect(held.glows).toBe(false);
      expect(held.fill).not.toBe(ground);
      expect(held.fill).not.toBe(owner.fill);
      expect(held.where?.words).toEqual(['M1', 'Check before the merge', 'paused']);

      // Stopped by the engine: filled and glowing, with no third row.
      expect(halted.glows).toBe(true);
      expect(halted.where).toBeNull();
    });
  }

  it('cuts what it waits for at the card, ending it with an ellipsis', () => {
    const long: Where = { cell: 'R2', title: 'Is there a better way', says: [['answer', 'group-trigger-cannot-be-dealt-with-at-all']] };
    scene.show([card(1, { kind: 'waits', at: 'R2', on: [{ kind: 'answer', question: 'x' }] }, long, 1)]);
    const words = scene.drawnCard(1)?.where?.words ?? [];
    expect(words.slice(0, 3)).toEqual(['R2', 'Is there a better way', 'answer']);
    expect(words[3].endsWith('…')).toBe(true);
    expect('group-trigger-cannot-be-dealt-with-at-all'.startsWith(words[3].slice(0, -1))).toBe(true);
  });
});
