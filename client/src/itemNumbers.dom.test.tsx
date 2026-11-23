// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Shell } from './App';
import { Markdown } from './console/markdown';
import { ItemTitlesProvider } from './itemNumbers';
import { INTERFACE, SourceContext } from './works/source';
import { fixtureItems } from './works/fixtures';
import type { Item } from './works-api';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
  localStorage.clear();
});

/// The items the page has read, of which only the numbers and the titles matter here.
const ITEMS = [
  { number: 46, title: 'Link the numbers' },
  { number: 47, title: 'Keep the tree' },
] as Item[];

function drawn(text: string): HTMLElement {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  act(() =>
    root.render(
      <ItemTitlesProvider items={{ items: ITEMS }}>
        <Markdown text={text} />
      </ItemTitlesProvider>,
    ),
  );
  unmount = () => act(() => root.unmount());
  return container;
}

/// What each link to an item of the console says, as it is written in the text.
const numbers = (page: ParentNode) =>
  [...page.querySelectorAll('a')]
    .filter((a) => a.getAttribute('href')?.startsWith('#/console/items/'))
    .map((a) => a.textContent);

describe('the number of an item in a text', () => {
  it('is a link to the item in the console that says its title, however it is written after an item word or with a #', () => {
    const page = drawn('See #46.');
    const link = page.querySelector('a');
    expect(link?.getAttribute('href')).toBe('#/console/items/46');
    expect(link?.getAttribute('title')).toBe('#46 Link the numbers');
    expect(link?.className).toBe('link mono');
    expect(page.textContent).toBe('See #46.');
    const written: [string, string[]][] = [
      ['#46', ['#46']],
      ['(#47)', ['#47']],
      ['条目 46', ['46']],
      ['item 47', ['47']],
      ['Items 46 and 47', ['46', '47']],
      ['条目 46、47', ['46', '47']],
      ['items 46, 47', ['46', '47']],
      ['items 46, and 47', ['46', '47']],
      // Chinese is written with no blanks, so a Han character never joins a number to a word.
      ['见#46的计划', ['#46']],
      ['条目46的背景', ['46']],
      ['依赖#47，再看条目 46。', ['#47', '46']],
    ];
    for (const [text, links] of written) {
      unmount?.();
      expect([text, numbers(drawn(text))]).toEqual([text, links]);
    }
  });

  it('is a link too in a list, a table, a heading and strong words, and stays the text it was', () => {
    const page = drawn('# Plan for #46\n\n- first #46\n- **item 47** next\n\n| a |\n|---|\n| #47 |\n');
    expect(numbers(page)).toEqual(['#46', '#46', '47', '#47']);
    expect(page.textContent).toBe('Plan for #46first #46item 47 nexta#47');
  });

  it('is no link where it is not an item number: in code, in a link, joined to a word or a path, after PR, or of no item', () => {
    const plain = [
      '`#46` and `item 46`',
      '[the item #46](https://example.com/46)',
      'PR #46',
      'pull request #46 and issue #47',
      'W46, v46, O46, a#46, lineitems 46',
      '53-27-23 and #46-1 and #46a and #46.5',
      'https://example.com/#46',
      '#046',
      '#9999 and item 9999 and 条目 3',
      '46 files, Article 8, round 47',
      '{{question}} and {{46}}',
    ];
    for (const text of plain) {
      unmount?.();
      const page = drawn(text);
      expect([text, numbers(page)]).toEqual([text, []]);
      expect(page.querySelectorAll('a a')).toHaveLength(0);
    }
  });

  it('links only the items that exist in a list, and the words of an explicit link stay its words', () => {
    expect(numbers(drawn('items 46, 9999 and 47'))).toEqual(['46', '47']);
    unmount?.();
    const page = drawn('[the item #46](https://example.com/46) then #47');
    expect(numbers(page)).toEqual(['#47']);
    expect(page.querySelector('a')?.textContent).toBe('the item #46');
  });

  it('is a link once the bar has read the items, in a view the shell holds', async () => {
    const [first] = fixtureItems().items;
    const source = {
      ...INTERFACE,
      loadItems: async () => fixtureItems(),
      loadEngine: undefined,
      changes: () => () => undefined,
    };
    const container = document.createElement('div');
    document.body.append(container);
    const root = createRoot(container);
    act(() =>
      root.render(
        <SourceContext.Provider value={source}>
          <Shell route={{ view: 'console' }}>
            <Markdown text={`See #${first.number}.`} />
          </Shell>
        </SourceContext.Provider>,
      ),
    );
    unmount = () => act(() => root.unmount());
    // Before the items are read, the number is the text it was written as.
    expect(numbers(container)).toEqual([]);
    for (let i = 0; i < 3; i += 1) {
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    }
    expect(numbers(container)).toEqual([`#${first.number}`]);
    expect(container.querySelector('.cw-md a')?.getAttribute('title')).toBe(`#${first.number} ${first.title}`);
  });
});
