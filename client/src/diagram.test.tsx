// A diagram fitted to its box and enlarged: the styles that keep a wide diagram from scrolling in its box, when a diagram
// counts as scaled down, which buttons stand in its corner, the enlarged view holding the one drawing under an id of its
// own, the view it opens at, and the spec saying all this. The tests have no DOM, so what only a browser lays out (the
// drawing shrinking in proportion, the dialog's size, the gestures inside it) is confirmed on the page itself.

import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import spec from '../ui-spec.md?raw';
import { DiagramTools, Enlarged, isClick, isScaled, under, type Drawn } from './diagram';
import { DRAG_LIMIT, fitView, seenOf } from './graph/Viewer';
import parts from './styles/parts.css?raw';
import { t } from './text';

// The viewer is the flow charts' own and needs a browser; here it shows the size it was given, and what it holds.
vi.mock('./graph/Viewer', async () => {
  const { createElement: element } = await import('react');
  return {
    // Its pure functions and constants are the real ones; only the component is stood in for.
    ...(await vi.importActual<typeof import('./graph/Viewer')>('./graph/Viewer')),
    Viewer: (p: { width: number; height: number; children: ReactNode }) =>
      element('div', { 'data-viewer': `${p.width}x${p.height}` }, p.children),
  };
});

/// The declarations of each rule of a style sheet one of whose selectors `pick` accepts, each on one line.
function rulesOf(css: string, pick: (selector: string) => boolean): string[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, '');
  return [...text.matchAll(/([^{}]*)\{([^{}]*)\}/g)]
    .filter((m) => m[1].split(',').some((s) => pick(s.trim())))
    .map((m) => m[2].replace(/\s+/g, ' ').trim());
}

describe('a diagram wider than its box', () => {
  it('is scaled down to the box in proportion and never scrolls in it', () => {
    const fit = rulesOf(parts, (s) => s === '.diagram-drawing > svg');
    expect(fit).toHaveLength(1);
    expect(fit[0]).toContain('max-width: 100%;');
    expect(fit[0]).toContain('height: auto;');
    // Of the diagram's rules, those found are the ones this looks at; none makes it scroll or keeps it from shrinking.
    const all = rulesOf(parts, (s) => s.startsWith('.diagram'));
    expect(all.length).toBeGreaterThan(8);
    for (const body of all) {
      expect(body).not.toMatch(/overflow(-x)?: (auto|scroll)/);
      expect(body).not.toContain('max-width: none');
    }
  });

  it('counts as scaled down only when its box has been measured and is narrower than the drawing', () => {
    expect(isScaled(1200, 378)).toBe(true);
    expect(isScaled(300, 378)).toBe(false);
    expect(isScaled(378, 378)).toBe(false);
    // A box not measured yet is no reason to offer the enlarged view.
    expect(isScaled(1200, 0)).toBe(false);
  });
});

describe("the buttons in a diagram's corner", () => {
  /// The buttons drawn, each as its label (the tooltip that labels it, or its own), and whether it is pressed.
  const buttons = (html: string): string[] =>
    [...html.matchAll(/<button\b([^>]*)>/g)].map((m) => {
      const by = /aria-labelledby="([^"]*)"/.exec(m[1]);
      const label = by !== null ? new RegExp(`id="${by[1]}">([^<]*)<`).exec(html) : /aria-label="([^"]*)"/.exec(m[1]);
      return `${label?.[1] ?? '?'}${/aria-pressed="true"/.test(m[1]) ? ' (pressed)' : ''}`;
    });
  const tools = (scaled: boolean, asCode: boolean) =>
    buttons(renderToStaticMarkup(createElement(DiagramTools, { scaled, asCode, onSource: () => {}, onZoom: () => {} })));

  it('are the enlarging one and the source one while the drawing is scaled down, the source one alone otherwise', () => {
    expect(tools(true, false)).toEqual([t('diagram_zoom'), t('diagram_source')]);
    expect(tools(false, false)).toEqual([t('diagram_source')]);
  });

  it('are the one that shows the diagram again alone while the source is shown, scaled down or not', () => {
    expect(tools(true, true)).toEqual([`${t('diagram_drawing')} (pressed)`]);
    expect(tools(false, true)).toEqual([`${t('diagram_drawing')} (pressed)`]);
  });
});

describe('the enlarged view', () => {
  // A drawing as the page keeps it, with the placeholder id mermaid's ids and styles are scoped by.
  const drawn: Drawn = {
    svg: '<svg id="wf-diagram-id" viewBox="0 0 1200 300" width="1200" height="300"><style>#wf-diagram-id .node{fill:red}</style><marker id="wf-diagram-id_arrow"></marker></svg>',
    width: 1200,
    height: 300,
  };

  it("holds the drawing it is given in a viewer of the drawing's own size, under an id of its own", () => {
    const html = renderToStaticMarkup(createElement(Enlarged, { drawn, id: 'wf-diagram-7' }));
    expect(html).toContain('data-viewer="1200x300"');
    expect(html).toContain(
      '<svg id="wf-diagram-7-zoom" viewBox="0 0 1200 300" width="1200" height="300"><style>#wf-diagram-7-zoom .node{fill:red}</style><marker id="wf-diagram-7-zoom_arrow"></marker></svg>',
    );
    // No id of the diagram's own, in the bubble, and no placeholder: two copies would share their styles and markers.
    expect(html).not.toMatch(/wf-diagram-7(?!-zoom)/);
    expect(html).not.toContain('wf-diagram-id');
    const bubble = under(drawn, 'wf-diagram-7');
    expect(bubble).toContain('<svg id="wf-diagram-7" ');
    expect(bubble).not.toContain('-zoom');
  });

  it('opens at the view that shows the whole drawing in the middle, no larger than 1.6 times', () => {
    const view = (box: [number, number], drawing: [number, number], want: [number, number, number]) => {
      const v = fitView(seenOf({ width: box[0], height: box[1] }, 0), { width: drawing[0], height: drawing[1] });
      expect(v.s).toBeCloseTo(want[0], 6);
      expect(v.x).toBeCloseTo(want[1], 6);
      expect(v.y).toBeCloseTo(want[2], 6);
    };
    // A box 832 by 532 holds 800 by 500 inside a margin of 16 on each side: the width binds, and the height is centred.
    view([832, 532], [800, 400], [1, 16, 66]);
    // A tall drawing is bound by the box's height, and centred across.
    view([832, 532], [400, 1000], [0.5, 316, 16]);
    // A small one is enlarged to 1.6 times and no more.
    view([1032, 532], [100, 50], [1.6, 436, 226]);
  });

  it('centres the whole drawing in what the window over the viewer leaves, as the viewer does', () => {
    // 232 of 1032 are covered, so 800 are seen; a drawing of 400 by 200 is bound by the width to 1.6 (capped), then
    // centred in the 800.
    const v = fitView(seenOf({ width: 1032, height: 532 }, 232), { width: 400, height: 200 });
    expect(v.s).toBeCloseTo(1.6, 6);
    expect(v.x).toBeCloseTo((800 - 640) / 2, 6);
    expect(v.y).toBeCloseTo((532 - 320) / 2, 6);
  });
});

describe('a click on the scaled drawing', () => {
  const at = (x: number, y: number) => ({ x, y });

  it('opens the enlarged view only when the pointer moved no further than the viewer lets a click move', () => {
    // The limit is the viewer's own, not a second number.
    expect(DRAG_LIMIT).toBe(6);
    expect(isClick(at(100, 100), at(100, 100))).toBe(true);
    // Exactly the limit is still a click.
    expect(isClick(at(100, 100), at(100 + DRAG_LIMIT, 100))).toBe(true);
    expect(isClick(at(100, 100), at(100, 100 - DRAG_LIMIT))).toBe(true);
    // Just beyond it, along an axis and along a diagonal, is a drag: selecting text does not open the window.
    expect(isClick(at(100, 100), at(100 + DRAG_LIMIT + 0.5, 100))).toBe(false);
    expect(isClick(at(100, 100), at(105, 105))).toBe(false);
  });
});

describe('the UI spec on diagrams', () => {
  it('tells of the scaling down and the enlarged view, not of scrolling in the box', () => {
    const section = spec.slice(spec.indexOf('### Diagrams'), spec.indexOf('### An item named by its number'));
    expect(section.length).toBeGreaterThan(1000);
    expect(section).toContain('scaled down to the width of its box');
    expect(section).toContain('opens enlarged');
    expect(section).not.toMatch(/scrolling inside its box|scrolls inside its box/);
    expect(section).toContain('a drag over the diagram does not open it');
  });
});
