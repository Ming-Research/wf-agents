// @vitest-environment happy-dom
import './dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { Markdown } from './console/markdown';
import { ItemTitlesProvider } from './itemNumbers';
import type { Item } from './works-api';

let unmount: (() => void) | undefined;
afterEach(() => {
  unmount?.();
  unmount = undefined;
  document.body.innerHTML = '';
});

const item = (number: number, title: string) => ({ number, title }) as Item;

/// Draws the text under the titles, and gives back a way to give the page other items.
function drawn(text: string, items: Item[] | null = [item(46, 'Link the numbers'), item(47, 'Keep the tree')]) {
  const container = document.createElement('div');
  document.body.append(container);
  const root = createRoot(container);
  const render = (now: Item[] | null) =>
    act(() =>
      root.render(
        <ItemTitlesProvider items={now === null ? null : { items: now }}>
          <Markdown text={text} />
        </ItemTitlesProvider>,
      ),
    );
  render(items);
  unmount = () => act(() => root.unmount());
  return { container, render };
}

const ADDRESS = '#/console/items/';
/// The links to an item of the console, as they are written in the text.
const numbers = (page: ParentNode) =>
  [...page.querySelectorAll('a')].filter((a) => a.getAttribute('href')?.startsWith(ADDRESS)).map((a) => a.textContent);

describe('the number of an item in a text, at the edges of the Markdown round it', () => {
  // Reference: CommonMark 0.31, ATX headings (a `#` run must be followed by a space or the end of the line, so `#46` is
  // a paragraph's words), block quotes, list items, and soft line breaks, which are whitespace.
  it('is a link at the start of a paragraph, a quote and a list item, where `#46` is not a heading', () => {
    expect(numbers(drawn('#46 is done').container)).toEqual(['#46']);
    unmount?.();
    const quoted = drawn('> #46 said so\n\n1. #47 first\n2. then 条目 46').container;
    expect(numbers(quoted)).toEqual(['#46', '#47', '46']);
    expect(quoted.querySelector('blockquote a')?.textContent).toBe('#46');
  });

  it('is a link where a line ends between the word and the number, and the text is left as it was written', () => {
    for (const text of ['条目\n46 已完成', 'item\n47 is done', 'items 46,\n47']) {
      unmount?.();
      const page = drawn(text).container;
      expect([text, numbers(page).length > 0]).toEqual([text, true]);
      expect(page.textContent?.replace(/\s+/g, ' ')).toBe(text.replace(/\s+/g, ' '));
    }
    unmount?.();
    expect(numbers(drawn('items 46,\n47').container)).toEqual(['46', '47']);
  });

  it('is a link inside stressed, strong and struck words, and each repeat of the number is a link', () => {
    expect(numbers(drawn('*item 46*, ~~#47~~ and **#46**: #46, #46 and #46').container)).toEqual([
      '46',
      '#47',
      '#46',
      '#46',
      '#46',
      '#46',
    ]);
  });

  // Reference: HTML 5.2, 4.5.1: an `a` element has no `a` element among its descendants. And the page's rule that an
  // address of a link is only the web or a view of the page (principle agent-writing-is-data). Each text is drawn with
  // the item links it must have, which says what became of the number in it: in the words of a link and in an address
  // it is no link; the words of a link whose target the page refuses are plain words, and the number in them is a link
  // to the item.
  it('draws no link inside a link and none to an address an agent wrote: a bare address, an address in angle brackets, a refused target', () => {
    const texts: [string, string[]][] = [
      ['see https://example.com/#46 and #47', ['#47']],
      ['see <https://example.com/#46> and #47', ['#47']],
      ['[#46](javascript:alert) and #47', ['#46', '#47']],
      ['[the item 46](https://example.com/x) and #47', ['#47']],
    ];
    for (const [text, links] of texts) {
      unmount?.();
      const page = drawn(text).container;
      expect([text, page.querySelectorAll('a a').length]).toEqual([text, 0]);
      const hrefs = [...page.querySelectorAll('a')].map((a) => a.getAttribute('href') ?? '');
      const strange = hrefs.filter((h) => !h.startsWith(ADDRESS) && !h.startsWith('https://example.com'));
      expect([text, strange]).toEqual([text, []]);
      expect([text, numbers(page)]).toEqual([text, links]);
    }
  });

  it('is not a link where it is too long for a number, is written in other digits, or is zero', () => {
    const page = drawn('#99999999999999999999999 and #４６ and #0 and item 0 and 条目 00').container;
    expect(numbers(page)).toEqual([]);
  });

  it('is not a link in a word that only ends in item, and is one after an item word in any case', () => {
    expect(numbers(drawn('subitem 46 and itemize 47 and my_item 46').container)).toEqual([]);
    unmount?.();
    expect(numbers(drawn('Item 46, ITEM 47 and ITEMS 46').container)).toEqual(['46', '47', '46']);
  });
});

describe('the title a link to an item says', () => {
  // Reference: principle agent-writing-is-data. A title is data from an agent: only ever the text of an attribute.
  it('is only the text of the title attribute, whatever the item is called', () => {
    const title = '<img src=x onerror="alert(1)"> "quoted" & <b>bold</b>';
    const page = drawn('See #46.', [item(46, title)]).container;
    expect(page.querySelector('a')?.getAttribute('title')).toBe(`#46 ${title}`);
    expect(page.querySelector('img, b')).toBeNull();
    expect(page.querySelector('a')?.getAttribute('onerror')).toBeNull();
  });

  it('follows the items the page reads: a title that changes is the new one, an item that is gone is no link, none read is the text', () => {
    const { container, render } = drawn('See #46 and 条目 47.');
    expect(numbers(container)).toEqual(['#46', '47']);
    render([item(46, 'Renamed'), item(47, 'Keep the tree')]);
    expect(container.querySelector('a')?.getAttribute('title')).toBe('#46 Renamed');
    render([item(47, 'Keep the tree')]);
    expect(numbers(container)).toEqual(['47']);
    render(null);
    expect(numbers(container)).toEqual([]);
    expect(container.textContent).toBe('See #46 and 条目 47.');
  });
});
