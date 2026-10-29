// The page against its UI spec (design/harness/owner-interface/ui-spec.md): every CSS file and every file of code, as
// ui-check.ts checks them, the exceptions each still matching something, and the check itself on what it must refuse
// and let through. `npm run check:ui` runs this file alone, as CI does; `npm test` leaves it to that step.

import { describe, expect, it } from 'vitest';

import { checkCode, checkCss, staleExceptions, type Finding } from './ui-check';

const name = (path: string) => path.replace(/^\.\//, '');

const css = Object.fromEntries(
  Object.entries(import.meta.glob<string>('./**/*.css', { query: '?raw', import: 'default', eager: true })).map(
    ([p, text]) => [name(p), text],
  ),
);

// The page's code, its tests and the check itself left out: they write raw values as the cases they are about.
const code = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(['./**/*.ts', './**/*.tsx', '!./**/*.test.ts', '!./**/*.test.tsx', '!./ui-check.ts'], {
      query: '?raw',
      import: 'default',
      eager: true,
    }),
  ).map(([p, text]) => [name(p), text]),
);

const said = (f: Finding) => `${f.file}:${f.line}: ${f.rule}: ${f.text}`;

describe('the page against its UI spec', () => {
  it('writes every colour, size and spacing as a token, and never makes muted text bold', () => {
    // The check reads the page's styles and code where they are, as they are written.
    expect(Object.keys(css)).toEqual(expect.arrayContaining(['styles.css', 'styles/base.css', 'styles/item.css']));
    expect(Object.keys(code)).toEqual(expect.arrayContaining(['main.tsx', 'pages/Item.tsx', 'graph/flowDraw.tsx']));
    expect(css['styles/base.css']).toContain('var(--fgColor-default)');
    expect(code['pages/Item.tsx']).toContain('PageHeader');
    const found = [
      ...Object.entries(css).flatMap(([file, text]) => checkCss(file, text)),
      ...Object.entries(code).flatMap(([file, text]) => checkCode(file, text)),
    ];
    expect(found.map(said)).toEqual([]);
  });

  it('has no exception that matches nothing', () => {
    expect(staleExceptions({ ...css, ...code })).toEqual([]);
  });
});

describe('the UI check', () => {
  it('refuses raw values in CSS and lets tokens through', () => {
    const raw = checkCss(
      'x.css',
      '.a {\n  color: #fff;\n  background: rgb(0 0 0 / 10%);\n  padding: 4px 8px;\n  gap: 6px;\n  font-size: 12px;\n' +
        '  font-weight: 600;\n  border: 1px solid var(--borderColor-default);\n  border-radius: 6px;\n' +
        '  box-shadow: 0 1px 2px var(--shadow-resting-small);\n  background: color-mix(in srgb, var(--a) 10%, transparent);\n}\n',
    );
    expect(raw.map((f) => f.line)).toEqual([2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    const tokens = checkCss(
      'x.css',
      '/* #fff in a comment */\n.b {\n  color: var(--fgColor-default);\n  padding: 0 var(--base-size-8);\n  margin: 0 auto;\n' +
        '  font: var(--text-body-shorthand-medium);\n  font-weight: var(--base-text-weight-semibold);\n' +
        '  border-bottom: var(--borderWidth-thin) solid var(--borderColor-muted);\n  border-radius: var(--borderRadius-medium);\n' +
        '  box-shadow: inset 0 calc(-1 * var(--borderWidth-thin)) 0 var(--borderColor-muted);\n  max-width: 60ch;\n}\n',
    );
    expect(tokens).toEqual([]);
  });

  it('refuses muted text made bold', () => {
    const found = checkCss('x.css', '.label {\n  color: var(--fgColor-muted);\n  font-weight: var(--base-text-weight-semibold);\n}\n');
    expect(found.map((f) => f.rule)).toEqual(['muted text is never bold']);
  });

  it('refuses raw values in inline styles and SVG attributes, and a font written out', () => {
    const found = checkCode(
      'x.tsx',
      "<div style={{ color: '#fff', padding: 4, fontSize: 12 }} />\n<rect fill=\"#000\" />\nconst f = '12px ui-sans-serif, sans-serif';\n",
    );
    expect(found.map((f) => f.rule)).toEqual([
      'colour: a Primer colour token, never a raw or mixed colour',
      'colour: a Primer colour token, never a raw or mixed colour',
      'type: a Primer text token',
      'spacing: a step of the size scale',
      'type: a Primer text token',
    ]);
    expect(checkCode('x.tsx', "<div style={{ width: `${pct}%`, paddingLeft: 'var(--base-size-8)' }} />\nfunction f(font: string) {}\n")).toEqual([]);
  });
});
