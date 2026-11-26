// Adversarial tests of the scanner that `raw-values.test.ts` holds. That file keeps the scanner inside itself and exports
// nothing, so this one reads the section between its two markers (`-- scanner`, `-- end scanner`), takes the types off
// it and runs it as it stands: what is tested is the code the check runs, not a copy of it. A marker moved or removed
// fails the first test, so a change of the file cannot leave these tests testing nothing.
//
// What each case is judged against, and not the scanner's own output:
//   - CSS Fonts Level 4 (§2.2 `font-weight`: `bold` is 700, `bolder` and `lighter` are weights too; §2.5 the
//     absolute-size and relative-size keywords of `font-size` are sizes; the `font` shorthand sets weight, size and
//     line height) and CSS Backgrounds Level 3 (`thin`, `medium`, `thick` are the lengths of a border's width): each
//     is a raw weight or size, as `700` and `13px` are, which the scanner already reports;
//   - CSS Media Queries Level 5 (`@custom-media` holds a media query) and CSS Cascade Level 5 (`@import` takes a media
//     query): a breakpoint written there is a breakpoint written out, as one in `@media` is.

import { transformWithEsbuild } from 'vite';
import { beforeAll, describe, expect, it } from 'vitest';

type Scan = (css: string) => { line: number; found: string; statement: string }[];

const SOURCE = import.meta.glob<string>('./raw-values.test.ts', { query: '?raw', import: 'default', eager: true });

let scan: Scan;

beforeAll(async () => {
  const text = Object.values(SOURCE)[0];
  const from = text.indexOf('// -- scanner');
  const to = text.indexOf('// -- end scanner');
  expect(from, 'the marker `-- scanner`').toBeGreaterThanOrEqual(0);
  expect(to, 'the marker `-- end scanner`').toBeGreaterThan(from);
  const { code } = await transformWithEsbuild(
    `function build() {\n${text.slice(from, to)}\nreturn { scan };\n}`,
    'scanner.ts',
    { loader: 'ts', target: 'es2022' },
  );
  scan = (new Function(`${code}\nreturn build();`)() as { scan: Scan }).scan;
});

const found = (css: string) => scan(css).map((f) => f.found).sort();

describe('the scanner of raw values: what a size or a weight written as a keyword is', () => {
  it('reports a weight written as a keyword, as it reports one written as a number', () => {
    expect(found('a { font-weight: 700; }')).toEqual(['700']);
    expect(found('a { font-weight: bold; }')).toEqual(['bold']);
    expect(found('a { font-weight: bolder; }')).toEqual(['bolder']);
    expect(found('a { font-weight: lighter; }')).toEqual(['lighter']);
  });

  it('reports a font size written as a keyword, as it reports one written in pixels', () => {
    expect(found('a { font-size: 13px; }')).toEqual(['13px']);
    expect(found('a { font-size: small; }')).toEqual(['small']);
    expect(found('a { font-size: x-large; }')).toEqual(['x-large']);
    expect(found('a { font-size: larger; }')).toEqual(['larger']);
  });

  it('reports the width of a border or an outline written as a keyword', () => {
    expect(found('a { border-width: thick; }')).toEqual(['thick']);
    expect(found('a { border: thin solid var(--line); }')).toEqual(['thin']);
    expect(found('a { outline: thick solid var(--accent); }')).toEqual(['thick']);
  });

  it('lets a keyword that is no size or weight stand, and every variable that names one', () => {
    expect(found('a { font: inherit; font-style: normal; font-variant: small-caps; }')).toEqual([]);
    expect(found('a { font-weight: var(--weight-strong); font-size: var(--size-body); font-size: inherit; }')).toEqual([]);
    expect(found('a { border: var(--hairline) solid var(--line); border: 0; outline: none; }')).toEqual([]);
    expect(found('a { font: var(--weight) var(--size-body) / var(--line-height) var(--font); }')).toEqual([]);
  });
});

describe('the scanner of raw values: the font shorthand', () => {
  it('reports a weight written in the shorthand, whose size is a variable', () => {
    expect(found('a { font: 600 var(--size-body) var(--font); }')).toEqual(['600']);
  });

  it('reports a line height written in the shorthand, whose size is a variable', () => {
    expect(found('a { font: var(--size-body) / 1.5 var(--font); }')).toEqual(['1.5']);
    expect(found('a { font: var(--size-body)/1.5 var(--font); }')).toEqual(['1.5']);
  });
});

describe('the scanner of raw values: a media condition in an at-rule that ends with a semicolon', () => {
  it('reports the breakpoint of a custom media query', () => {
    expect(found('@custom-media --narrow (max-width: 30em);')).toEqual(['30em']);
    expect(found('@custom-media --narrow (width < 760px);')).toEqual(['760px']);
  });

  it('reports the breakpoint of an import', () => {
    expect(found('@import "narrow.css" (max-width: 30em);')).toEqual(['30em']);
  });
});

describe('the scanner of raw values: what it already gets right in the cases that are easy to get wrong', () => {
  it('reports a colour given to a function that mixes or picks colours', () => {
    expect(found('a { color: color-mix(in srgb, var(--a) 40%, white); }')).toEqual(['white']);
    expect(found('a { color: light-dark(#fff, #000); }')).toEqual(['#000', '#fff']);
    expect(found('a { color: color-mix(in srgb, var(--a) 40%, transparent); }')).toEqual([]);
  });

  it('reads a value that has no semicolon before the brace that closes, at any depth', () => {
    expect(found('a {\n  b {\n    width: 5px\n  }\n  top: 6px\n}')).toEqual(['5px', '6px']);
    expect(found('a{width:5px;;height:6px}')).toEqual(['5px', '6px']);
  });

  it('reads the property and the unit in capitals, and a value that is important', () => {
    expect(found('a { WIDTH: 5PX !important; }')).toEqual(['5PX']);
    expect(found('@MEDIA (min-width: 40em) { a { top: 0 } }')).toEqual(['40em']);
  });

  it('reads a negative number that starts with a point, and a length in a media range', () => {
    expect(found('a { margin: 0 -.5em; }')).toEqual(['-.5em']);
    expect(found('@media (400px <= width <= 700px) { a { top: 0 } }')).toEqual(['400px', '700px']);
  });

  it('keeps a brace or a quote inside a comment or a string from ending or beginning a rule', () => {
    expect(found('a { width: 5px; } /* } */ b { width: 6px }')).toEqual(['5px', '6px']);
    expect(found('/* \' */ a { width: 5px; }')).toEqual(['5px']);
    expect(found('a::after { content: "}{;"; width: 5px }')).toEqual(['5px']);
  });

  it('does not read what a feature query asks about', () => {
    expect(found('@supports (width: 5px) { a { top: 0 } }')).toEqual([]);
  });
});
