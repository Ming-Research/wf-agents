// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { LEVEL_LOOKS, LevelMark, levelLook, levelPointed } from './marks';
import { NO_LEVEL } from './works/words';

afterEach(() => {
  document.body.innerHTML = '';
});

/// The mark of a level as the item's window draws it: its words, its colour, its weight, and what pointing at it says.
function drawn(props: Parameters<typeof LevelMark>[0]) {
  const container = document.body.appendChild(document.createElement('div'));
  act(() => createRoot(container).render(<LevelMark {...props} />));
  const mark = container.querySelector('span') as HTMLElement;
  return { words: mark.textContent, colour: mark.style.color, weight: mark.style.fontWeight, title: mark.title };
}

describe('the level a cell runs at', () => {
  it('is written as the item level plus the levels the cell really runs above it, in the look of the level it runs at', () => {
    const raised = drawn({ level: '3', raise: 1, held: null, cellLevel: '4' });
    expect(raised.words).toBe('L3+1');
    expect(raised.colour).toBe(`var(${levelLook('4', 0).colour})`);
    expect(raised.weight).toBe('var(--weight-strong)');
    expect(raised.title).toBe('level 3, levels_above 1');
  });

  it('is written as the level it really runs at where a setting held it, and says what held it on pointing', () => {
    // Item at 3, its line asks one more, the highest a round may run at is 3: it runs at 3, as a cell not raised.
    const held = drawn({ level: '3', raise: 0, held: { asks: 1, by: 'agents.highest_level' }, cellLevel: '3' });
    expect(held.words).toBe('L3');
    expect(held.colour).toBe(`var(${LEVEL_LOOKS[2].colour})`);
    expect(held.weight).toBe('');
    expect(held.title).toBe('level 3, levels_above 1, agents.highest_level 3');
    // Held part of the way: the line asks two, one was got.
    const part = drawn({ level: '2', raise: 1, held: { asks: 2, by: 'agents.highest_level' }, cellLevel: '3' });
    expect(part.words).toBe('L2+1');
    expect(part.title).toBe('level 2, levels_above 2, agents.highest_level 3');
  });

  it('says its level alone where its line does not raise it, and why there is none where the engine gives none', () => {
    expect(drawn({ level: '2', raise: 0 }).title).toBe('level 2');
    expect(drawn({ level: null, raise: 0 })).toMatchObject({ words: NO_LEVEL.shown, title: NO_LEVEL.level });
    expect(levelPointed('3', 0, { asks: 1, by: null }, '3')).toBe('level 3, levels_above 1');
  });
});
