// The way a work went, as the head of an item says it: a sentence for the two ways that are not through the design,
// each with the hover text that tells what it leaves out or why the work went on as it did, and nothing for any other
// value, including the marks and routes of the flow versions before.

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { routeFact } from './pages/Item';

const html = (route_taken: string | null) => {
  const fact = routeFact({ route_taken });
  return fact === null ? null : renderToStaticMarkup(createElement('div', null, fact));
};

describe('the hover text of the way a work went', () => {
  it('has a hover text for a work implemented straight from its research, and another for one turned', () => {
    const direct = html('direct');
    const turned = html('turned');
    expect(direct).toMatch(/<span title="[^"]+">/);
    expect(turned).toMatch(/<span title="[^"]+">/);
    const titleOf = (h: string | null) => /title="([^"]+)"/.exec(h ?? '')?.[1];
    // Each way explains itself in words of its own.
    expect(titleOf(direct)).not.toEqual(titleOf(turned));
  });

  it('says nothing for a value that is not a way a work goes now', () => {
    for (const old of ['light', 'normal', 'to_normal', 'done', '', 'Direct']) {
      expect(html(old)).toBeNull();
    }
  });
});
