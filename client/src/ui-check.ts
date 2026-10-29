// The check of the page against its UI spec (design/harness/owner-interface/ui-spec.md), which `npm run check:ui` runs
// in CI: in the page's CSS and in the inline styles of its code, every colour, font size, font, weight, spacing,
// radius and border width is a var() of a Primer token or of the page's own token file, and no rule makes muted text
// bold. What the check cannot see, such as whether a thing uses the part the spec maps it to, a reviewer checks.

/// Something the check refuses: where, which rule of the spec, and the text that breaks it.
export type Finding = { file: string; line: number; rule: string; text: string };

/// The page's own tokens, for the values Primer has none for: the one place raw values are written.
export const TOKEN_FILE = 'styles/tokens.css';

/// What the check leaves alone, each with why: a whole file (no `text`) or one value in it. The list only shrinks: a
/// view converted to the spec takes its file off it, and the check refuses an entry that matches nothing.
export const EXCEPTIONS: Array<{ file: string; text?: string; why: string }> = [
  {
    file: 'styles.css',
    why:
      "The styles of what a cell's tabs and a round's panel hold (with the JSON and file text they show), which a " +
      'redesign of their own converts, leaving it for a file under styles/ when it is converted.',
  },
];

const COLOUR_FUNCTION = /\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color-mix|color)\(/;
const HEX = /#[0-9a-fA-F]{3,8}\b/;
const NAMED_COLOUR =
  /(^|[\s,(])(white|black|red|green|blue|gray|grey|orange|yellow|purple|pink|brown|silver|gold|navy|teal|maroon|olive|lime|aqua|fuchsia|cyan|magenta)(?=$|[\s,)])/i;
const LENGTH = /(^|[^\w-])-?(\d*\.)?\d+(px|rem|em|ex|ch|pt|pc|cm|mm|in|vw|vh|vmin|vmax|%)(?![\w-])/;
const NUMBER = /(^|[^\w-])-?(\d*\.)?\d+(?![\w-])/;

/// A value with every var() taken out, its fallback included, so that what is left is what was written raw.
function bare(value: string): string {
  let out = value;
  for (;;) {
    const at = out.indexOf('var(');
    if (at < 0) return out;
    let depth = 0;
    let end = at + 3;
    for (; end < out.length; end++) {
      if (out[end] === '(') depth++;
      else if (out[end] === ')' && --depth === 0) break;
    }
    out = `${out.slice(0, at)}${out.slice(end + 1)}`;
  }
}

const KEYWORD = /^(inherit|initial|unset|revert)$/;

/// What a declaration breaks, if anything: the rule of the spec it breaks, by its property and its value.
function declaration(prop: string, value: string): string | null {
  const p = prop.toLowerCase();
  const raw = bare(value).trim();
  if (p.startsWith('--')) return null;
  if (HEX.test(raw) || COLOUR_FUNCTION.test(raw) || (/color|background|border|outline|fill|stroke|shadow/.test(p) && NAMED_COLOUR.test(raw))) {
    return 'colour: a Primer colour token, never a raw or mixed colour';
  }
  if (p === 'font-size' || p === 'font' || p === 'font-family') {
    return raw === '' || KEYWORD.test(raw) ? null : 'type: a Primer text token';
  }
  if (p === 'font-weight') return raw === '' || KEYWORD.test(raw) ? null : 'weight: a Primer weight token';
  if (/^(margin|padding)(-|$)|^(row-|column-)?gap$/.test(p)) {
    return LENGTH.test(raw) ? 'spacing: a step of the size scale' : null;
  }
  if (/^border(-(top|right|bottom|left|inline|block)(-(start|end))?)?(-width)?$/.test(p) || p === 'outline' || p === 'outline-width' || p === 'box-shadow') {
    return LENGTH.test(raw) ? 'border: a border width token' : null;
  }
  if (/radius$/.test(p)) return LENGTH.test(raw) || NUMBER.test(raw.replace(/^0$/, '')) ? 'radius: a radius token' : null;
  return null;
}

/// The line a position of a text is on.
const lineAt = (text: string, at: number) => text.slice(0, at).split('\n').length;

/// Whether a finding at a line of a file is one the exceptions name.
function excepted(file: string, source: string, at: number): boolean {
  return EXCEPTIONS.some((e) => {
    if (e.file !== file) return false;
    if (e.text === undefined) return true;
    const i = source.indexOf(e.text);
    return i >= 0 && i <= at && at < i + e.text.length + 1;
  });
}

/// What a CSS file breaks: each declaration of a raw value, and each rule that makes muted text bold.
export function checkCss(file: string, source: string): Finding[] {
  if (file === TOKEN_FILE) return [];
  const out: Finding[] = [];
  // Comments become spaces, so that positions stay where they are.
  const text = source.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, ' '));
  const block = /([^{}]*)\{([^{}]*)\}/g;
  for (let m = block.exec(text); m !== null; m = block.exec(text)) {
    const bodyAt = m.index + m[1].length + 1;
    const decls = [...m[2].matchAll(/([-\w]+)\s*:\s*([^;]+)/g)];
    let muted = false;
    let bold: number | null = null;
    for (const d of decls) {
      const at = bodyAt + (d.index ?? 0);
      const prop = d[1];
      const value = d[2].trim();
      const rule = declaration(prop, value);
      if (rule !== null && !excepted(file, source, at)) out.push({ file, line: lineAt(text, at), rule, text: `${prop}: ${value}` });
      if (prop === 'color' && value.includes('--fgColor-muted')) muted = true;
      if (prop === 'font-weight' && /semibold|medium|bold|[5-9]00/.test(value)) bold = at;
    }
    if (muted && bold !== null && !excepted(file, source, bold)) {
      out.push({ file, line: lineAt(text, bold), rule: 'muted text is never bold', text: m[1].trim() });
    }
  }
  return out;
}

/// What the page's code breaks: a raw colour, font size, font or spacing in an inline style, a raw colour in an SVG
/// attribute, and a font written out in a string.
export function checkCode(file: string, source: string): Finding[] {
  const out: Finding[] = [];
  const add = (at: number, rule: string, text: string) => {
    if (!excepted(file, source, at)) out.push({ file, line: lineAt(source, at), rule, text });
  };
  const colourKey = /\b(color|background|backgroundColor|borderColor|fill|stroke|stopColor|outlineColor)\s*[:=]\s*\{?\s*['"`](#|rgb|hsl|color-mix)/g;
  for (const m of source.matchAll(colourKey)) add(m.index ?? 0, 'colour: a Primer colour token, never a raw or mixed colour', m[0]);
  for (const m of source.matchAll(/\b(fontSize|fontWeight|fontFamily)\s*:\s*(\d|['"`](?!var\())[^,}\n]*/g)) {
    add(m.index ?? 0, 'type: a Primer text token', m[0]);
  }
  for (const m of source.matchAll(/\b(padding|margin|gap|rowGap|columnGap)(Top|Right|Bottom|Left|Inline|Block|InlineStart|InlineEnd|BlockStart|BlockEnd)?\s*:\s*(?!['"`]?var\(|0\b)['"`]?-?\d[^,}\n]*/g)) {
    add(m.index ?? 0, 'spacing: a step of the size scale', m[0]);
  }
  for (const m of source.matchAll(/['"`][^'"`\n]*\b\d+(\.\d+)?px\s[^'"`\n]*\b(sans-serif|serif|monospace)\b[^'"`\n]*['"`]/g)) {
    add((m.index ?? 0) + 1, 'type: a Primer text token', m[0]);
  }
  return out;
}

/// The exceptions that match nothing in the files given, so that the list cannot outlive what it excepts.
export function staleExceptions(files: Record<string, string>): string[] {
  return EXCEPTIONS.filter((e) => !(e.file in files) || (e.text !== undefined && !files[e.file].includes(e.text))).map(
    (e) => `${e.file}${e.text !== undefined ? `: ${e.text}` : ''}`,
  );
}
