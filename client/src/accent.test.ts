// The accent's tones and its refusals, in Node: how a colour is measured, held against published values; that the
// default turns to itself and every preset to itself; that every colour taken gives tones that read on their grounds in
// light and in dark at least as well as the default's do, in its own hue; that a colour near one of a meaning, or grey,
// is refused, the meaning named; and that the tokens write the tones and the meanings as the accent reads them.

import { describe, expect, it } from 'vitest';

import {
  APART,
  declared,
  DEFAULT_ACCENT,
  DEFAULT_TONES,
  GREY,
  MEANINGS,
  PRESETS,
  refusal,
  strength,
  type Theme,
  TONES,
  tones,
  turning,
} from './accent';
import { apart, contrast, hexOf, labOf, lchOf } from './colour';

const LIGHT = declared(':root {');
const DARK = declared(":root[data-theme='dark'] {");
const SYSTEM_DARK = declared(":root:not([data-theme='light']) {");

const THEMES: Theme[] = ['light', 'dark'];
const GROUNDS: Record<Theme, { card: string; ground: string; text: string }> = {
  light: { card: LIGHT.card, ground: LIGHT.ground, text: LIGHT.text },
  dark: { card: DARK.card, ground: DARK.ground, text: DARK.text },
};

/// How far two colours stand apart in OKLab.
const distance = (a: string, b: string) => {
  const [p, q] = [labOf(a), labOf(b)];
  return Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);
};

/// Colours across the hues the accent may take, each at several strengths: the presets, and every fifth degree of hue
/// at the default's lightness, strong, middling and weak.
const colours = [
  ...PRESETS.map((p) => p.colour),
  ...Array.from({ length: 72 }, (_, i) => i * 5).flatMap((h) => [0.3, 0.16, 0.1].map((c) => hexOf(0.6, c, h))),
  '#e9d5ff',
  '#1e3a8a',
];
const taken = colours.filter((c) => refusal(c) === null);

describe('how a colour is measured', () => {
  it("gives WCAG's contrast: 21 for black on white, and the greys either side of 4.5", () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 6);
    expect(contrast('#ffffff', '#ffffff')).toBe(1);
    expect(contrast('#767676', '#ffffff')).toBeCloseTo(4.54, 2);
    expect(contrast('#777777', '#ffffff')).toBeCloseTo(4.48, 2);
  });

  it("gives the OKLCH of sRGB's red, green and blue as published, and holds every colour it makes in sRGB", () => {
    const near = (hex: string, l: number, c: number, h: number) => {
      const got = lchOf(hex);
      expect(got.l).toBeCloseTo(l, 4);
      expect(got.c).toBeCloseTo(c, 4);
      expect(got.h).toBeCloseTo(h, 2);
    };
    near('#ff0000', 0.62796, 0.25768, 29.23);
    near('#00ff00', 0.86644, 0.29483, 142.5);
    near('#0000ff', 0.45201, 0.31321, 264.05);
    for (const hex of ['#8b5cf6', '#1f2937', '#f6f7f9', '#e5484d', '#14a3a8']) {
      const { l, c, h } = lchOf(hex);
      expect(hexOf(l, c, h)).toBe(hex);
    }
    // Asked for more chroma than sRGB holds, it keeps the lightness and the hue and lowers the chroma.
    const wide = lchOf(hexOf(0.6, 0.5, 264));
    expect(wide.l).toBeCloseTo(0.6, 2);
    expect(apart(wide.h, 264)).toBeLessThan(1);
    expect(wide.c).toBeLessThan(0.5);
    expect(apart(350, 10)).toBe(20);
  });
});

describe("the accent's tones", () => {
  it('are the default tones for the default accent, and every preset is the accent it gives in light', () => {
    expect(tones(DEFAULT_ACCENT)).toEqual(DEFAULT_TONES);
    const turn = turning(DEFAULT_ACCENT);
    expect(turn('#6d9bf9', 'read')).toBe('#6d9bf9');
    expect(turning(null)('#d5e2ff', 'tint')).toBe('#d5e2ff');
    for (const p of PRESETS) expect(tones(p.colour).light.accent).toBe(p.colour);
    expect(PRESETS.map((p) => p.name)).toEqual(['Blue', 'Indigo', 'Violet', 'Purple']);
    expect(PRESETS[0].colour).toBe(DEFAULT_ACCENT);
  });

  it("read on their grounds, in light and in dark, at least as well as the default, in the colour's hue", () => {
    expect(taken.length).toBeGreaterThan(40);
    for (const colour of taken) {
      const given = tones(colour);
      for (const theme of THEMES) {
        const { card, ground, text } = GROUNDS[theme];
        const t = given[theme];
        const p = DEFAULT_TONES[theme];
        const turn = lchOf(colour).h - lchOf(DEFAULT_ACCENT).h;
        for (const [tone, kind] of Object.entries(TONES[theme])) {
          // Each tone is the default's turned by as many degrees as the colour stands from the default accent, within
          // what rounding to eight bits moves the hue of a tone of little chroma.
          expect(apart(lchOf(t[tone]).h, lchOf(p[tone]).h + turn), `${colour} ${theme} ${tone}`).toBeLessThan(4);
          if (kind === 'read') {
            expect(contrast(t[tone], card)).toBeGreaterThanOrEqual(contrast(p[tone], card) - 1e-9);
            expect(contrast(t[tone], ground)).toBeGreaterThanOrEqual(contrast(p[tone], ground) - 1e-9);
          }
        }
        // The accent marks and rings on the card, the ground and its own tint; words in it read on all three.
        const words = theme === 'light' ? t['accent-text'] : t.accent;
        for (const on of [card, ground, t['accent-tint']]) {
          expect(contrast(t.accent, on), `${colour} ${theme} accent`).toBeGreaterThanOrEqual(3);
          expect(contrast(words, on), `${colour} ${theme} words`).toBeGreaterThanOrEqual(4.5);
          expect(contrast(text, on)).toBeGreaterThanOrEqual(4.5);
        }
        // The heads stand over the accent's words; the cell running stands apart from the cell done.
        if (theme === 'light') expect(contrast(t.head, card)).toBeGreaterThan(contrast(t['accent-text'], card));
        expect(contrast(t['mark-running'], t['mark-done'])).toBeGreaterThanOrEqual(
          0.9 * contrast(p['mark-running'], p['mark-done']),
        );
        // The tint stands from the card no farther than the default's, and not much nearer.
        expect(contrast(t['accent-tint'], card)).toBeLessThanOrEqual(contrast(p['accent-tint'], card) + 1e-9);
        expect(distance(t['accent-tint'], card)).toBeGreaterThanOrEqual(0.8 * distance(p['accent-tint'], card));
      }
    }
  });
});

describe('a colour refused as the accent', () => {
  it('is one near the colour of a meaning, the nearest named, or one too grey', () => {
    expect(refusal(LIGHT.red)).toBe('red');
    expect(refusal(DARK.red)).toBe('red');
    expect(refusal('#ec4899')).toBe('red');
    expect(refusal(LIGHT.orange)).toBe('orange');
    expect(refusal(LIGHT.yellow)).toBe('yellow');
    expect(refusal(LIGHT.green)).toBe('green');
    expect(refusal('#ff00f0')).toBe('red');
    expect(refusal('#64748b')).toBe('grey');
    expect(refusal('#808080')).toBe('grey');
    expect(refusal('#ffffff')).toBe('grey');
    // A near white or a near black has a hue no more than rounding: grey, however little chroma sRGB holds there.
    for (const near of ['#fefeff', '#fafaff', '#eeeeff', '#000001', '#0a0a0c'])
      expect(refusal(near), near).toBe('grey');
    // Pale or dark, a colour of a free hue is taken by its hue and strength.
    expect(refusal('#e9d5ff')).toBeNull();
    expect(refusal('#1e3a8a')).toBeNull();
  });

  it('is never a preset, and is every colour a tone of which comes within the bound of a meaning', () => {
    for (const p of PRESETS) expect(refusal(p.colour), p.name).toBeNull();
    const hues = (Object.values(MEANINGS) as readonly (readonly string[])[]).flatMap((names) =>
      names.flatMap((n) => [lchOf(LIGHT[n]).h, lchOf(DARK[n] ?? LIGHT[n]).h]),
    );
    for (const colour of colours) {
      const given = tones(colour);
      const near = THEMES.some((theme) =>
        Object.values(given[theme]).some((t) => hues.some((h) => apart(lchOf(t).h, h) < APART)),
      );
      const grey = strength(colour) < GREY * strength(DEFAULT_ACCENT);
      expect(refusal(colour) !== null, colour).toBe(near || grey);
    }
  });
});

describe('the tokens', () => {
  it("write each of the accent's tones as its own or the default's, and the system's dark as the switch's", () => {
    const own = (block: Record<string, string>) =>
      Object.entries(block)
        .filter(([, v]) => v.includes('--own-'))
        .map(([k]) => k)
        .sort();
    expect(own(LIGHT)).toEqual(Object.keys(TONES.light).sort());
    expect(own(DARK)).toEqual(Object.keys(TONES.dark).sort());
    expect(SYSTEM_DARK).toEqual(DARK);
    expect(LIGHT.running).toBe('var(--accent)');
    expect(LIGHT['running-text']).toBe('var(--accent-text)');
    expect(DARK['accent-text']).toBe('var(--accent)');
    expect(DARK.head).toBe('var(--accent)');
    expect(DARK.running).toBeUndefined();
  });

  it('give every colour of theirs a place: a tone of the accent, what follows it, a grey or a ground, or a meaning the accent keeps clear of', () => {
    const accent = [...Object.keys(TONES.light), 'running', 'running-text', 'track-model'];
    const neutral = [
      'ground',
      'card',
      'text',
      'muted',
      'line',
      'step',
      'window-shadow',
      'scrim',
      'dialog-shadow',
      'on-colour',
      'track-ground',
      'track-gap',
    ];
    // The tools' time follows a meaning: it is drawn in the yellow of a call. The wait for the project's turn to check and
    // merge is a colour of its own, a teal in the band of hues around the green that the accent keeps clear of.
    const meanings = [...(Object.values(MEANINGS).flat() as string[]), 'track-tool', 'track-turn'];
    // What tells things of one kind apart and says how nothing stands: the lines of the flow in a chart of them, and the
    // kinds of the words of code. The accent keeps clear of the meanings alone (decision
    // an-accent-keeps-clear-of-the-meanings).
    const kinds = ['flow-line-1', 'flow-line-2', 'flow-line-3', 'flow-line-4', 'flow-line-5', 'code-keyword', 'code-literal'];
    const placed = new Set([...accent, ...neutral, ...meanings, ...kinds]);
    // A token is a colour where its value is one, in any of CSS's notations, or names a token that is one.
    const notation = /#[0-9a-f]{3,8}\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color|color-mix|light-dark)\(/i;
    const isColour = (block: Record<string, string>, v: string, depth = 0): boolean =>
      notation.test(v) ||
      (depth < 8 &&
        [...v.matchAll(/var\(--([a-z0-9-]+)/g)].some(([, n]) => {
          const named = block[n] ?? LIGHT[n];
          return named !== undefined && isColour(block, named, depth + 1);
        }));
    const unplaced = (block: Record<string, string>) =>
      Object.entries(block)
        .filter(([k, v]) => isColour(block, v) && !placed.has(k))
        .map(([k]) => k);
    // A colour of a new meaning, in any notation, or one that names the accent or a tone of it, is one the check
    // finds; a size is none.
    expect(
      unplaced({
        ...LIGHT,
        pink: '#ff69b4',
        sky: 'oklch(0.7 0.1 230)',
        'chosen-line': 'var(--accent)',
        'done-line': 'var(--mark-done)',
        gap: '4px',
      }),
    ).toEqual(['pink', 'sky', 'chosen-line', 'done-line']);
    for (const block of [LIGHT, DARK]) {
      expect(Object.keys(block).length).toBeGreaterThan(20);
      expect(unplaced(block)).toEqual([]);
    }
  });
});
