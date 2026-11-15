// The accent's tones and its refusals, in Node: how a colour is measured, held against published values; that the
// purple turns to itself and every preset to itself; that every colour taken gives tones that read on their grounds in
// light and in dark at least as well as the purple's do, in its own hue; that a colour near one of a meaning, or grey,
// is refused, the meaning named; and that the tokens write the tones and the meanings as the accent reads them.

import { describe, expect, it } from 'vitest';

import {
  APART,
  declared,
  GREY,
  MEANINGS,
  PRESETS,
  PURPLE,
  refusal,
  strength,
  type Theme,
  TONES,
  tones,
  turning,
  VIOLET,
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
/// at the purple's lightness, strong, middling and weak.
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
  it('are the purple for the page\'s own violet, and every preset is the accent it gives in light', () => {
    expect(tones(VIOLET)).toEqual(PURPLE);
    const turn = turning(VIOLET);
    expect(turn('#a78bfa', 'read')).toBe('#a78bfa');
    expect(turning(null)('#e4dcfd', 'tint')).toBe('#e4dcfd');
    for (const p of PRESETS) expect(tones(p.colour).light.accent).toBe(p.colour);
    expect(PRESETS.map((p) => p.name)).toEqual(['Blue', 'Indigo', 'Violet', 'Purple']);
  });

  it('read on their grounds, in light and in dark, at least as well as the purple, in the hue of the colour taken', () => {
    expect(taken.length).toBeGreaterThan(40);
    for (const colour of taken) {
      const given = tones(colour);
      for (const theme of THEMES) {
        const { card, ground, text } = GROUNDS[theme];
        const t = given[theme];
        const p = PURPLE[theme];
        const turn = lchOf(colour).h - lchOf(VIOLET).h;
        for (const [tone, kind] of Object.entries(TONES[theme])) {
          // Each tone is the purple's turned by as many degrees as the colour stands from the violet, within what
          // rounding to eight bits moves the hue of a tone of little chroma.
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
        // The tint stands from the card no farther than the purple's, and not much nearer.
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
    expect(refusal(LIGHT['track-tool'])).toBe('tools');
    expect(refusal('#0ea5e9')).toBe('tools');
    expect(refusal('#64748b')).toBe('grey');
    expect(refusal('#808080')).toBe('grey');
    expect(refusal('#ffffff')).toBe('grey');
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
      const grey = strength(colour) < GREY * strength(VIOLET);
      expect(refusal(colour) !== null, colour).toBe(near || grey);
    }
  });
});

describe('the tokens', () => {
  it("write each of the accent's tones as its own or the purple, and the system's dark as the switch's", () => {
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

  it('give every colour of theirs a place: a tone of the accent, what follows it, a grey or a ground, the way back, or a meaning the accent keeps clear of', () => {
    const accent = [...Object.keys(TONES.light), 'running', 'running-text', 'track-model'];
    const neutral = ['ground', 'card', 'text', 'muted', 'line', 'step', 'window-shadow', 'on-colour', 'track-ground', 'track-gap'];
    const meanings = Object.values(MEANINGS).flat() as string[];
    const placed = new Set([...accent, ...neutral, 'violet', ...meanings]);
    // A token is a colour where its value is one, or is worked out from one.
    const isColour = (v: string) => /#[0-9a-f]{3,8}\b|rgb\(|color-mix\(|var\(--(accent|running|own-)/i.test(v);
    for (const block of [LIGHT, DARK]) {
      const unplaced = Object.entries(block)
        .filter(([k, v]) => isColour(v) && !placed.has(k))
        .map(([k]) => k);
      expect(unplaced).toEqual([]);
    }
  });
});
