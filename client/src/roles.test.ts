// A role has one colour wherever the page shows it: the colour the flow gives its line, drawn through one lookup, the
// class of that colour in roles.css, which sets the four tokens a chart's cell, a bubble and an avatar are drawn with;
// an agent with no colour of its own the page's neutral grey, the engine's own steps' on a chart.

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Colour, GraphLines } from './api';
import { Bubble } from './pages/Bubble';
import { coloursOf, readColours, roleClass } from './roles';
import chartCss from './styles/chart.css?raw';
import rolesCss from './styles/roles.css?raw';

/// Every colour the flow may give a line, which TypeScript holds to the colours the flow's compiler knows: a colour
/// added there and not here, or here and not there, does not type-check.
const KNOWN: Record<Colour, true> = { blue: true, teal: true, auburn: true, pink: true, purple: true, indigo: true, lime: true };

/// The declarations of a rule in a stylesheet, by property, its comments left out.
function rule(stylesheet: string, selector: string): Map<string, string> {
  const css = stylesheet.replace(/\/\*[\s\S]*?\*\//g, '');
  const at = css.indexOf(`${selector} {`);
  expect(at, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(css.indexOf('{', at) + 1, css.indexOf('}', at));
  return new Map(
    body
      .split(';')
      .map((d) => d.trim())
      .filter((d) => d !== '')
      .map((d): [string, string] => [d.slice(0, d.indexOf(':')).trim(), d.slice(d.indexOf(':') + 1).trim()]),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("a role's colour", () => {
  // Read as the page starts, the lines are there before any bubble is drawn, which is then drawn in its role's colour
  // and not in the grey of an agent with no colour of its own; asked for again while being read, they are read once.
  it('is read as the page starts, before a bubble is drawn, and once', async () => {
    const lines: GraphLines[] = [{ graph: 'engineer', lines: [{ name: 'engineer', desc: 'The engineer', colour: 'blue' }] }];
    const asked: string[] = [];
    vi.stubGlobal('fetch', async (path: string) => {
      asked.push(path);
      return new Response(JSON.stringify(lines), { status: 200, headers: { 'Content-Type': 'application/json' } });
    });
    const drawn = () => renderToStaticMarkup(createElement(Bubble, { who: 'engineer', head: 'Engineer', children: 'Said.' }));
    expect(drawn()).toContain('talk-bubble talk-bubble-agent role-none');
    readColours();
    readColours();
    await vi.waitFor(() => expect(drawn()).toContain('talk-bubble talk-bubble-agent role-blue'));
    expect(asked).toEqual(['/api/v1/flows/lines']);
  });

  it("is the class of the colour the flow gives its line, or the neutral grey's for none", () => {
    const colours = coloursOf([
      {
        graph: 'engineer',
        lines: [
          { name: 'engineer', desc: 'The engineer', colour: 'blue' },
          { name: 'review', desc: 'The review', colour: 'teal' },
        ],
      },
      { graph: 'architect', lines: [{ name: 'architect', desc: 'The architect', colour: 'purple' }] },
    ]);
    expect(roleClass(colours, 'engineer')).toBe('role-blue');
    expect(roleClass(colours, 'review')).toBe('role-teal');
    expect(roleClass(colours, 'architect')).toBe('role-purple');
    // A line the flow no longer has, no line, and any line while the lines are not read yet.
    expect(roleClass(colours, 'scout')).toBe('role-none');
    expect(roleClass(colours, null)).toBe('role-none');
    expect(roleClass(coloursOf([]), 'engineer')).toBe('role-none');
  });

  it('has a class for each colour the flow may give, setting the four Primer display tokens of that colour', () => {
    for (const name of Object.keys(KNOWN)) {
      expect(Object.fromEntries(rule(rolesCss, `.role-${name}`)), name).toEqual({
        '--role-edge': `var(--display-${name}-borderColor-emphasis)`,
        '--role-fill': `var(--display-${name}-bgColor-muted)`,
        '--role-line': `var(--display-${name}-borderColor-muted)`,
        '--role-mark': `var(--display-${name}-bgColor-emphasis)`,
      });
    }
  });

  it("draws an agent with no colour of its own in the engine's grey, the page's neutral", () => {
    const none = rule(rolesCss, '.role-none');
    const chart = rule(chartCss, ':root');
    expect(none.get('--role-edge')).toBe('var(--borderColor-neutral-emphasis)');
    expect(none.get('--role-fill')).toBe('var(--bgColor-neutral-muted)');
    expect(chart.get('--fc-engine')).toBe(none.get('--role-edge'));
    expect(chart.get('--fc-engine-fill')).toBe(none.get('--role-fill'));
  });
});
