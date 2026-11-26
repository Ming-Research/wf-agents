// The check of principle no-raw-values-in-styles: no stylesheet of the page but tokens.css writes a raw colour, size or
// space; every other style uses the named values of tokens.css. A value that cannot be a token is listed in
// `EXCEPTIONS` with its reason.
//
// The scanner reads the text of a stylesheet and not a parsed tree: the page's styles are plain declarations, and a
// parser would be a new dependency (a change of the lock file). It visits each declaration (`property: value`, at any
// depth: inside `@media`, `@keyframes` and nested rules) and the conditions of `@media` and `@container`, and reports
// in a value:
//   - a colour written out: `#hex`, a colour function (`rgb(`, `hsl(`, `oklch(`, ...) or a colour's name;
//   - a length: any number but zero with a unit that is not on the allowed list (`%`, `fr`, viewport, time and angle
//     units), so that a unit not thought of is reported;
//   - a unitless number in a property that holds a size or a weight (`stroke-width`, `line-height`, ...).
// What a `var()` names is not read, and what it holds as a fallback is.

import { describe, expect, it } from 'vitest';

// -- scanner

type Finding = { line: number; found: string; statement: string };

const COLOUR_NAMES = new Set(
  (
    'aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood ' +
    'cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod ' +
    'darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon ' +
    'darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray ' +
    'dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green ' +
    'greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon ' +
    'lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon ' +
    'lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta ' +
    'maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen ' +
    'mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive ' +
    'olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru ' +
    'pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen ' +
    'seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato ' +
    'turquoise violet wheat white whitesmoke yellow yellowgreen'
  ).split(' '),
);

// The units that say a share of something, or a time or an angle, and no size that the look names.
const ALLOWED_UNITS = new Set([
  '%',
  'fr',
  ...['vw', 'vh', 'vi', 'vb', 'vmin', 'vmax'].flatMap((u) => [u, `s${u}`, `l${u}`, `d${u}`]),
  's',
  'ms',
  'deg',
  'rad',
  'turn',
  'grad',
]);

// The properties whose value is a size or a weight when it is a number with no unit.
const UNITLESS_SIZES = new Set([
  'stroke-width',
  'stroke-dasharray',
  'stroke-dashoffset',
  'line-height',
  'font-weight',
]);

// Blanks what is not style (comments, strings, the insides of `url()`), keeping every length and every line break, so
// that an offset in the result is an offset in the text.
function blank(css: string): string {
  const noise = /\/\*[\s\S]*?\*\/|url\([^)]*\)|"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*'/g;
  return css.replace(noise, (m) => m.replace(/[^\n]/g, ' '));
}

function scan(css: string): Finding[] {
  const text = blank(css);
  const findings: Finding[] = [];
  const lineAt = (offset: number) => text.slice(0, offset).split('\n').length;

  const look = (value: string, at: number, prop: string, statement: string) => {
    const report = (index: number, found: string) =>
      findings.push({ line: lineAt(at + index), found, statement });
    for (const m of value.matchAll(/#[0-9a-fA-F]{3,8}(?![\w-])/g)) report(m.index, m[0]);
    for (const m of value.matchAll(/(?<![\w-])(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/gi)) report(m.index, m[0]);
    for (const m of value.matchAll(/(?<![\w.#-])[a-z]+(?![\w(-])/gi)) {
      if (COLOUR_NAMES.has(m[0].toLowerCase())) report(m.index, m[0]);
    }
    for (const m of value.matchAll(/(?<![\w.#-])[+-]?(\d+\.?\d*|\.\d+)([a-z%]*)/gi)) {
      const unit = m[2].toLowerCase();
      if (Number.parseFloat(m[1]) === 0) continue;
      if (unit === '' ? UNITLESS_SIZES.has(prop) : !ALLOWED_UNITS.has(unit)) report(m.index, m[0]);
    }
  };

  let start = 0;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (c !== '{' && c !== ';' && c !== '}') continue;
    const statement = text.slice(start, i);
    if (c === '{') {
      // A rule's head: a selector says no style; the condition of a media or container query holds sizes.
      if (/^\s*@(media|container)\b/i.test(statement)) look(statement, start, '', statement.trim().replace(/\s+/g, ' '));
    } else {
      const colon = statement.indexOf(':');
      const prop = statement.slice(0, colon).trim().toLowerCase();
      if (colon > 0 && /^-{0,2}[a-z][\w-]*$/.test(prop)) {
        look(statement.slice(colon + 1), start + colon + 1, prop, statement.trim().replace(/\s+/g, ' '));
      }
    }
    start = i + 1;
  }
  return findings;
}

// A value the check lets stand, with why, and the decision that makes the choice. It stands only where its reason holds:
// in a statement that begins with `within` (an at-rule such as `@media`), so that the same value in an ordinary
// declaration of the same file is still reported.
type Exception = { file: string; value: string; within: string; reason: string; decision: string };

// What the sheets hold that is not an exception, and each exception that no sheet holds any longer.
function judge(sheets: Record<string, string>, exceptions: Exception[]) {
  const found = Object.entries(sheets).flatMap(([file, css]) => scan(css).map((f) => ({ file, ...f })));
  const holds = (e: Exception, f: { file: string; found: string; statement: string }) =>
    e.file === f.file && e.value === f.found && f.statement.toLowerCase().startsWith(e.within);
  return {
    unlisted: found.filter((f) => !exceptions.some((e) => holds(e, f))),
    stale: exceptions.filter((e) => !found.some((f) => holds(e, f))),
  };
}

// The sheets that must be among those read, so that a read that found too little does not pass for a clean one.
function missing(files: string[], required: string[]): string[] {
  return required.filter((r) => !files.includes(r));
}

// -- end scanner

const TOKENS = 'styles/tokens.css';
const REQUIRED = [TOKENS, 'styles/parts.css', 'console/console.css'];

// A media condition cannot read a custom property, so its breakpoints cannot be named values.
const MEDIA_REASON = 'a media condition cannot read a custom property, so a breakpoint cannot be a named value';
const MEDIA_DECISION = 'the-style-check-lists-the-media-breakpoints';
const EXCEPTIONS: Exception[] = ['1200px', '760px', '480px'].map((value) => ({
  file: 'styles/works.css',
  value,
  within: '@media',
  reason: MEDIA_REASON,
  decision: MEDIA_DECISION,
}));

// Every stylesheet of the page, by its path under src/ (the glob names them from the root of the page's project, so that
// a sheet beside this test is named as one in another directory is).
const SHEETS: Record<string, string> = Object.fromEntries(
  Object.entries(import.meta.glob<string>('/src/**/*.css', { query: '?raw', import: 'default', eager: true })).map(
    ([path, css]) => [path.replace(/^\/src\//, ''), css],
  ),
);

const found = (css: string) => scan(css).map((f) => f.found);

describe('the check of raw values in the page’s styles', () => {
  it('finds no raw colour, size or space in a stylesheet but tokens.css, beside what is listed with its reason', () => {
    const others = Object.fromEntries(Object.entries(SHEETS).filter(([file]) => file !== TOKENS));
    const { unlisted, stale } = judge(others, EXCEPTIONS);
    expect(unlisted.map((f) => `${f.file}:${f.line} ${f.found} in "${f.statement}"`)).toEqual([]);
    expect(stale.map((e) => `${e.file} ${e.value}`)).toEqual([]);
  });

  it('reads the stylesheets it must, so that a read that found too little fails', () => {
    expect(missing(Object.keys(SHEETS), REQUIRED)).toEqual([]);
  });

  it.each(REQUIRED)('fails when %s is not among the stylesheets read', (file) => {
    const without = Object.keys(SHEETS).filter((f) => f !== file);
    expect(missing(without, REQUIRED)).toEqual([file]);
  });

  it('gives each exception the decision that makes its choice and a reason', () => {
    for (const e of EXCEPTIONS) {
      expect(e.reason, `${e.file} ${e.value}`).not.toBe('');
      expect(e.within, `${e.file} ${e.value}`).toMatch(/^@[a-z-]+$/);
      expect(e.decision, `${e.file} ${e.value}`).toMatch(/^[a-z]+(-[a-z]+)*$/);
    }
  });
});

describe('the scanner of raw values', () => {
  it('reports a colour in hexadecimal', () => {
    expect(found('a { color: #3673f5; }')).toEqual(['#3673f5']);
    expect(found('a { color: #fff; }')).toEqual(['#fff']);
  });

  it('reports a colour function, also inside a shadow', () => {
    expect(found('a { background: rgb(31 35 40 / 35%); }')).toEqual(['rgb(']);
    expect(found('a { box-shadow: 0 0 var(--glow-blur) oklch(60% 0.2 250); }')).toEqual(['oklch(']);
  });

  it('reports a colour by its name, and not a name inside a variable’s or a keyword', () => {
    expect(found('a { color: red; }')).toEqual(['red']);
    expect(found('a { border: var(--hairline) solid white; }')).toEqual(['white']);
    expect(found('a { color: var(--red); background: var(--green-text); }')).toEqual([]);
    expect(found('a { background: transparent; color: currentcolor; }')).toEqual([]);
  });

  it('reports a length in px, em and ch, and a negative one', () => {
    expect(found('a { width: 480px; }')).toEqual(['480px']);
    expect(found('a { max-height: calc(3 * var(--line-height) * 1em); }')).toEqual(['1em']);
    expect(found('a { width: 5ch; margin-left: -3px; }')).toEqual(['5ch', '-3px']);
    expect(found('a { padding: 0 0.5rem; }')).toEqual(['0.5rem']);
  });

  it('reports a unit it has no rule for, as it is not on the allowed list', () => {
    expect(found('a { width: 12Q; }')).toEqual(['12Q']);
    expect(found('a { width: 3pc; }')).toEqual(['3pc']);
  });

  it('reports a length held as the fallback of a variable', () => {
    expect(found('a { padding-right: var(--window-covers, 12px); }')).toEqual(['12px']);
  });

  it('reports a length in a shadow, a gradient and a custom property declared outside tokens.css', () => {
    expect(found('a { box-shadow: 0 8px 16px var(--shade); }')).toEqual(['8px', '16px']);
    expect(found('a { background: repeating-linear-gradient(45deg, var(--x) 0 2px, transparent 2px 5px); }')).toEqual([
      '2px',
      '2px',
      '5px',
    ]);
    expect(found(':root { --popup: 600px; }')).toEqual(['600px']);
  });

  it('reports a number with no unit in a property that holds a size or a weight', () => {
    expect(found('a { stroke-width: 2; }')).toEqual(['2']);
    expect(found('a { stroke-dasharray: 4 3; }')).toEqual(['4', '3']);
    expect(found('a { line-height: 1.5; }')).toEqual(['1.5']);
    expect(found('a { font-weight: 600; }')).toEqual(['600']);
  });

  it('reports a size in the condition of a media or container query', () => {
    expect(found('@media (width < 1200px) { a { display: none; } }')).toEqual(['1200px']);
    expect(found('@container (min-width: 30em) { a { display: none; } }')).toEqual(['30em']);
  });

  it('reports a value in a block inside a block, as in keyframes and nested rules', () => {
    expect(found('@keyframes k { from { top: 10px; } to { top: 0; } }')).toEqual(['10px']);
    expect(found('a { b { color: #000; } }')).toEqual(['#000']);
  });

  it('lets a variable, zero, a share, a viewport size, a time and an angle stand', () => {
    expect(found('a { width: var(--space-16); height: 0; margin: 0px 0.0em; }')).toEqual([]);
    expect(found('a { width: 100%; grid-template-columns: 1fr minmax(0, 2fr); height: 100vh; width: 50dvw; }')).toEqual(
      [],
    );
    expect(found('a { transition: opacity 240ms ease; animation-delay: 1s; transform: rotate(45deg); }')).toEqual([]);
    expect(found('a { padding: calc(var(--space-16) + var(--space-4)); width: calc(100vw - var(--space-16) * 4); }')).toEqual(
      [],
    );
  });

  it('lets a number with no unit stand where it is no size, and a variable where it is', () => {
    expect(found('a { opacity: 0.5; flex: 1 1 0; z-index: 3; transition: x 1s cubic-bezier(0.2, 0.8, 0.2, 1); }')).toEqual(
      [],
    );
    expect(found('a { line-height: var(--line-height); font-weight: var(--weight-strong); stroke-width: var(--w); }')).toEqual(
      [],
    );
  });

  it('does not read a comment, a string, an url or a selector', () => {
    expect(found('/* a gap of 12px in #fff */ a { color: var(--text); }')).toEqual([]);
    expect(found('a::before { content: "12px #fff red"; }')).toEqual([]);
    expect(found('a { background: url("x.png#12px"); }')).toEqual([]);
    expect(found('.a-5px, #fff-box, red:hover { display: block; }')).toEqual([]);
    expect(found('a { font-family: "SF Pro Text", system-ui, sans-serif; }')).toEqual([]);
  });

  it('gives the line of the value that was found, also in a declaration on several lines', () => {
    const css = ['a {', '  color: var(--text);', '  box-shadow:', '    0 0 0 var(--hairline) var(--line),', '    0 8px 16px var(--shade);', '}'].join('\n');
    expect(scan(css).map((f) => [f.line, f.found])).toEqual([
      [5, '8px'],
      [5, '16px'],
    ]);
    expect(scan('@media (width < 760px) {\n  a { top: 4px; }\n}').map((f) => [f.line, f.found])).toEqual([
      [1, '760px'],
      [2, '4px'],
    ]);
  });

  const media760 = { file: 'a.css', value: '760px', within: '@media', reason: 'r', decision: 'd' };

  it('holds back an exception that is listed, and reports what is not', () => {
    const sheets = { 'a.css': '@media (width < 760px) { b { top: 0; } }\n@media (width < 480px) { b { top: 0; } }' };
    const { unlisted, stale } = judge(sheets, [media760]);
    expect(unlisted.map((f) => f.found)).toEqual(['480px']);
    expect(stale).toEqual([]);
  });

  it('reports an exception that no stylesheet holds any longer', () => {
    const { unlisted, stale } = judge({ 'a.css': 'b { top: 0; }' }, [media760]);
    expect(unlisted).toEqual([]);
    expect(stale).toEqual([media760]);
  });

  it('names an exception by its file as well as its value', () => {
    const { unlisted } = judge({ 'b.css': '@media (width < 760px) { c { top: 0; } }' }, [media760]);
    expect(unlisted.map((f) => `${f.file} ${f.found}`)).toEqual(['b.css 760px']);
  });

  it('lets an exception stand only where its reason holds, so the same value in a declaration is reported', () => {
    const sheet = '@media (width < 760px) { b { top: 0; } }\nc { max-width: 760px; }';
    const { unlisted, stale } = judge({ 'a.css': sheet }, [media760]);
    expect(unlisted.map((f) => `${f.line} ${f.found}`)).toEqual(['2 760px']);
    expect(stale).toEqual([]);
  });

  it('reports an exception as no longer held when its value stands only in a declaration', () => {
    const { unlisted, stale } = judge({ 'a.css': 'c { max-width: 760px; }' }, [media760]);
    expect(unlisted.map((f) => f.found)).toEqual(['760px']);
    expect(stale).toEqual([media760]);
  });
});
