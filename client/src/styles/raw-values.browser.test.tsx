// A second reading of the page's stylesheets for raw values, by a different route from the check of `raw-values.test.ts`:
// Chromium parses each sheet (`CSSStyleSheet.replaceSync`), and this test walks the rules it made (style rules, nested
// rules, keyframes, `@media` and `@container`) and reads each declaration as the browser serialises it. So what the
// check's own reading of the text could miss (where a rule ends, a comment, a string, a nested block) is read here by the
// browser's parser, and the question whether a word is a colour or a unit is a length is asked of the browser
// (`CSS.supports`), not of a list written in the test. It judges by the same rule as the principle
// no-raw-values-in-styles: no colour, length or line size written out outside tokens.css, but the three `@media`
// breakpoints of works.css, which a media condition cannot read from a custom property.

import { describe, expect, it } from 'vitest';

const SHEETS: Record<string, string> = Object.fromEntries(
  Object.entries(import.meta.glob<string>('/src/**/*.css', { query: '?raw', import: 'default', eager: true })).map(
    ([path, css]) => [path.replace(/^\/src\//, ''), css],
  ),
);

// Words a colour property takes that name no colour of their own.
const NOT_A_COLOUR = new Set(['inherit', 'initial', 'unset', 'revert', 'revert-layer', 'transparent', 'currentcolor']);
// The system colours (CSS Color Level 4, §6.2): the platform's own, which the page does not choose.
const SYSTEM_COLOURS = new Set(
  (
    'accentcolor accentcolortext activetext buttonborder buttonface buttontext canvas canvastext field fieldtext ' +
    'graytext highlight highlighttext linktext mark marktext selecteditem selecteditemtext visitedtext'
  ).split(' '),
);
// A unit that says a share of something and no size of its own: a percentage and the viewport's.
const SHARE = /^(?:%|[sld]?v(?:w|h|i|b|min|max))$/i;
// The properties whose value is a size or a weight when it is a number with no unit (the font shorthand sets both).
const UNITLESS_SIZES = new Set([
  'stroke-width',
  'stroke-dasharray',
  'stroke-dashoffset',
  'line-height',
  'font-weight',
  'font',
]);
// A size or a weight a keyword names (CSS Fonts Level 4 §2.2 and §2.5, CSS Backgrounds Level 3).
const KEYWORD_SIZES: Record<string, RegExp> = {
  'font-weight': /(?<![\w-])(?:bold|bolder|lighter)(?![\w-])/gi,
  'font-size': /(?<![\w-])(?:(?:xx?x?-)?(?:small|large)|medium|larger|smaller)(?![\w-])/gi,
  border: /(?<![\w-])(?:thin|medium|thick)(?![\w-])/gi,
  'border-width': /(?<![\w-])(?:thin|medium|thick)(?![\w-])/gi,
  outline: /(?<![\w-])(?:thin|medium|thick)(?![\w-])/gi,
  'outline-width': /(?<![\w-])(?:thin|medium|thick)(?![\w-])/gi,
};

/// The raw colours, lengths and line sizes a value holds, as the text of each.
function rawIn(value: string, property: string): string[] {
  const text = value.replace(/url\([^)]*\)|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g, ' ');
  const raw: string[] = [];
  for (const m of text.matchAll(/#[0-9a-f]{3,8}(?![\w-])/gi)) raw.push(m[0]);
  for (const m of text.matchAll(/(?<![\w-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/gi)) raw.push(m[0]);
  for (const m of text.matchAll(/(?<![\w#.-])-?[a-z][a-z-]*(?![\w(-])/gi)) {
    const word = m[0].toLowerCase();
    if (NOT_A_COLOUR.has(word) || SYSTEM_COLOURS.has(word)) continue;
    if (CSS.supports('color', word)) raw.push(m[0]);
  }
  for (const m of text.matchAll(/(?<![\w#.-])[+-]?(?:\d+\.?\d*|\.\d+)([a-z%]*)/gi)) {
    if (Number.parseFloat(m[0]) === 0) continue;
    const unit = m[1];
    if (unit === '' ? UNITLESS_SIZES.has(property) : !SHARE.test(unit) && CSS.supports('width', `1${unit}`)) raw.push(m[0]);
  }
  const keyword = KEYWORD_SIZES[property];
  if (keyword !== undefined) for (const m of text.matchAll(keyword)) raw.push(m[0]);
  return raw;
}

/// What the rules of a sheet hold that is raw, each as `where: text`; a media or container condition says its sizes as
/// `@media` or `@container`.
function rawInRules(rules: CSSRuleList): string[] {
  const out: string[] = [];
  for (const rule of Array.from(rules)) {
    if (rule instanceof CSSMediaRule) {
      for (const r of rawIn(rule.media.mediaText, '')) out.push(`@media: ${r}`);
    } else if (typeof CSSContainerRule !== 'undefined' && rule instanceof CSSContainerRule) {
      for (const r of rawIn(rule.conditionText, '')) out.push(`@container: ${r}`);
    }
    const style = (rule as Partial<CSSStyleRule>).style;
    if (style !== undefined) {
      const where = (rule as Partial<CSSStyleRule>).selectorText ?? rule.constructor.name;
      for (const d of style.cssText.matchAll(/(?:^|;)\s*([-\w]+)\s*:\s*([^;]*)/g)) {
        for (const r of rawIn(d[2], d[1].toLowerCase())) out.push(`${where} { ${d[1]}: ${r} }`);
      }
    }
    const inside = (rule as Partial<CSSGroupingRule>).cssRules;
    if (inside !== undefined) out.push(...rawInRules(inside));
  }
  return out;
}

function rawInSheet(css: string): string[] {
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(css);
  return rawInRules(sheet.cssRules);
}

describe('a second reading of the page’s stylesheets by the browser’s own parser', () => {
  it('finds a raw colour, length or line size in the oracle’s sample of every kind, in every place a rule can be', () => {
    const found = rawInSheet(`
      a { --c: #123456; color: #654321; background: rgb(1 2 3); border-color: red; width: 5px; margin: 0 -.5em; }
      a { stroke-width: 2; font-weight: 600; font: 600 var(--s) x; box-shadow: 0 0 1Q var(--x); }
      a { line-height: 1.5; }
      a { padding: var(--a, 12px); font-weight: bold; font-size: small; border: thin solid var(--l); }
      @media (width < 700px) { b { top: 3ch } }
      @container (min-width: 30em) { b { top: 0 } }
      @keyframes k { to { top: 7px } }
      c { d { top: 8px } left: 9px }
      /* a { width: 99px } */
      e::after { content: "11px #fff red"; }
    `);
    const text = found.join('\n');
    for (const one of [
      '#123456',
      'rgb(',
      'red',
      '5px',
      '-0.5em',
      '2 }',
      '1.5',
      '600',
      '1Q',
      '12px',
      'bold',
      'small',
      'thin',
      '@media: 700px',
      '3ch',
      '@container: 30em',
      '7px',
      '8px',
      '9px',
    ]) {
      expect(text, one).toContain(one);
    }
    expect(text).not.toContain('99px');
    expect(text).not.toContain('11px');
  });

  it('finds nothing raw in what a variable, zero, a share, a time or an angle gives', () => {
    expect(
      rawInSheet(`
        a { width: var(--w); height: 0; margin: 0px; padding: 50%; height: 100vh; transition: opacity 240ms ease; }
        a { transform: rotate(45deg); color: transparent; background: currentcolor; font: inherit; line-height: var(--l); }
        a { width: calc(100vw - var(--space-16) * 2); border: var(--hairline) solid var(--line); }
      `),
    ).toEqual([]);
  });

  it('reads tokens.css, where the raw values are, as full of them: the reading is not blind', () => {
    const found = rawInSheet(SHEETS['styles/tokens.css']);
    expect(found.length).toBeGreaterThan(40);
    expect(found.filter((r) => /: #[0-9a-f]{3,8} \}/i.test(r)).length).toBeGreaterThan(10);
    expect(found.filter((r) => /: \d+px \}/.test(r)).length).toBeGreaterThan(10);
  });

  it('finds, in every other stylesheet, only the three media breakpoints of works.css', () => {
    const found = Object.entries(SHEETS)
      .filter(([file]) => file !== 'styles/tokens.css')
      .flatMap(([file, css]) => rawInSheet(css).map((r) => `${file} ${r}`))
      .sort();
    expect(found).toEqual(['styles/works.css @media: 1200px', 'styles/works.css @media: 480px', 'styles/works.css @media: 760px']);
  });
});
