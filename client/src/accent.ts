// The accent: the page's own violet, or a colour the owner picked in the Settings view, kept in this browser as light
// or dark is (theme.ts). The tokens (styles/tokens.css) write each tone of the accent as its own one or the purple, in
// light and in dark; a colour picked turns every purple tone to its hue and strength, each tone keeping the purple's
// lightness, made lighter or darker only where it would read less well on its ground than the purple does, and the
// page sets the tones so turned on the root. A colour that would be taken for one that means something else is refused,
// and the accent stays as it was.

import { useSyncExternalStore } from 'react';

import { apart, contrast, hexOf, lchOf, widest } from './colour';
import tokens from './styles/tokens.css?raw';

export type Theme = 'light' | 'dark';

const THEMES: Theme[] = ['light', 'dark'];

/// How a tone is read: a mark, a ring or words, on the card and the ground (`read`); or a fill that words stand on
/// (`tint`).
export type Kind = 'read' | 'tint';

/// The tones of the accent, by their names among the tokens, in light and in dark, and how each is read. The tokens
/// write each as `--own-<tone>` (or `--own-<tone>-dark`) or the purple; in dark, the accent's text and the heads are the
/// accent itself, and what runs is the accent in both.
export const TONES: Record<Theme, Record<string, Kind>> = {
  light: {
    accent: 'read',
    'accent-text': 'read',
    'accent-tint': 'tint',
    head: 'read',
    'mark-done': 'read',
    'mark-running': 'read',
  },
  dark: { accent: 'read', 'accent-tint': 'tint', 'mark-done': 'read', 'mark-running': 'read' },
};

/// The colours with a meaning of their own, which the accent keeps clear of, by the tokens that draw each: red, a
/// problem; orange, a block's meaning changed; yellow, a call under way; green, a work done; teal, the tools' time.
export const MEANINGS = {
  red: ['red', 'red-text'],
  orange: ['orange', 'orange-text'],
  yellow: ['yellow'],
  green: ['green', 'green-text'],
  tools: ['track-tool'],
} as const;

export type Meaning = keyof typeof MEANINGS;

/// Why a colour is not taken as the accent: too near the colour of a meaning, or too grey.
export type Refusal = Meaning | 'grey';

/// How many degrees of hue every tone of the accent stands at least from each colour of a meaning, in light and in
/// dark; and the least strength of a colour taken, as a part of the purple's. Both by judgment: within 45 degrees a
/// faint tint or a small mark read as the meaning's colour, and below 0.4 of the purple's strength the accent reads as
/// the muted grey of words and of a pause.
export const APART = 45;
export const GREY = 0.4;

/// The custom properties the tokens declare in the block that `opener` begins, by name with no dashes, each with its
/// value as written: the root's in light (`:root {`), and in dark as the bar's switch sets it
/// (`:root[data-theme='dark'] {`) or as the system's (`:root:not([data-theme='light']) {`).
export function declared(opener: string): Record<string, string> {
  const css = tokens.replace(/\/\*[\s\S]*?\*\//g, '');
  const at = css.indexOf(opener);
  if (at < 0) throw new Error(`the tokens have no block ${opener}`);
  const from = at + opener.length;
  const body = css.slice(from, css.indexOf('}', from));
  return Object.fromEntries(
    [...body.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1], m[2].trim().replace(/\s+/g, ' ')]),
  );
}

const BLOCKS: Record<Theme, Record<string, string>> = {
  light: declared(':root {'),
  dark: declared(":root[data-theme='dark'] {"),
};

/// The colour a token has, in light or dark, where the tokens write it `#rrggbb`: the dark block names only what it
/// changes.
function colourOf(theme: Theme, name: string): string {
  const v = BLOCKS[theme][name] ?? BLOCKS.light[name];
  if (v === undefined || !/^#[0-9a-f]{6}$/i.test(v)) throw new Error(`the token --${name} is not a colour #rrggbb`);
  return v.toLowerCase();
}

/// The name a tone's own is set by on the root, in light or dark.
export const ownName = (theme: Theme, tone: string) => (theme === 'light' ? `--own-${tone}` : `--own-${tone}-dark`);

/// Each tone's purple, as the tokens write it in the place of its own.
function purpleOf(theme: Theme, tone: string): string {
  const own = ownName(theme, tone);
  const m = /^var\((--[a-z-]+), (#[0-9a-f]{6})\)$/i.exec(BLOCKS[theme][tone] ?? '');
  if (m === null || m[1] !== own) throw new Error(`the token --${tone} is not written as ${own} or its purple`);
  return m[2].toLowerCase();
}

export const PURPLE: Record<Theme, Record<string, string>> = {
  light: Object.fromEntries(Object.keys(TONES.light).map((t) => [t, purpleOf('light', t)])),
  dark: Object.fromEntries(Object.keys(TONES.dark).map((t) => [t, purpleOf('dark', t)])),
};

/// The page's own accent: the purple's, in light.
export const VIOLET = PURPLE.light.accent;

/// The card and the ground of each theme, what the tones are read on.
const GROUNDS: Record<Theme, { card: string; ground: string }> = {
  light: { card: colourOf('light', 'card'), ground: colourOf('light', 'ground') },
  dark: { card: colourOf('dark', 'card'), ground: colourOf('dark', 'ground') },
};

/// The hue of each colour of a meaning, in light and in dark.
const MEANING_HUES: [Meaning, number][] = (Object.keys(MEANINGS) as Meaning[]).flatMap((m) =>
  THEMES.flatMap((theme) => MEANINGS[m].map((name): [Meaning, number] => [m, lchOf(colourOf(theme, name)).h])),
);

/// The lightness, from the darkest to the lightest, at which a colour's strength is measured: towards black and white
/// sRGB holds almost no chroma, so that a tinge of a near white or a near black, its hue no more than rounding, would
/// count as a strong colour. By judgment: within them a pale or a dark colour of a hue is measured as itself.
const MEASURED = [0.25, 0.85] as const;

/// How strong a colour is: its chroma as a part of the most sRGB holds at its hue and its lightness, the lightness
/// taken within `MEASURED`.
export function strength(colour: string): number {
  const { l, c, h } = lchOf(colour);
  return c / widest(Math.min(MEASURED[1], Math.max(MEASURED[0], l)), h);
}

/// How a colour turns the purple: by how many degrees of hue, and how much stronger or weaker.
type Turn = { hue: number; strength: number };

function turnOf(colour: string): Turn {
  return { hue: lchOf(colour).h - lchOf(VIOLET).h, strength: strength(colour) / strength(VIOLET) };
}

/// A purple tone turned: at the purple's lightness, its hue turned and its strength scaled (a tint keeps its chroma,
/// which is how far it stands from the card); then made darker in light, lighter in dark, until it reads on the card
/// and the ground of `theme` at least as well as the purple does, or, a tint, lighter in light and darker in dark until
/// it stands from the card no farther than the purple's.
function turned(purple: string, kind: Kind, theme: Theme, turn: Turn): string {
  const { l, c, h } = lchOf(purple);
  const hue = h + turn.hue;
  const share = (c / widest(l, h)) * turn.strength;
  const at = (lightness: number) =>
    hexOf(lightness, kind === 'tint' ? c : Math.min(1, share) * widest(lightness, hue), hue);
  const { card, ground } = GROUNDS[theme];
  const near = 1e-9;
  const reads = (x: string) =>
    kind === 'tint'
      ? contrast(x, card) <= contrast(purple, card) + near
      : contrast(x, card) >= contrast(purple, card) - near && contrast(x, ground) >= contrast(purple, ground) - near;
  const step = (theme === 'light') === (kind === 'read') ? -0.001 : 0.001;
  let lightness = l;
  let x = at(lightness);
  for (let i = 0; i < 400 && !reads(x); i += 1) {
    lightness += step;
    x = at(lightness);
  }
  return x;
}

/// The tones of the accent a colour gives, in light and in dark.
export function tones(colour: string): Record<Theme, Record<string, string>> {
  const turn = turnOf(colour);
  const of = (theme: Theme) =>
    Object.fromEntries(
      Object.entries(TONES[theme]).map(([tone, kind]) => [tone, turned(PURPLE[theme][tone], kind, theme, turn)]),
    );
  return { light: of('light'), dark: of('dark') };
}

/// How the console's own purples, drawn in light, turn for `colour`: each as a tone of the accent is, on the light
/// card and ground; as they are for the page's own violet or none.
export function turning(colour: string | null): (purple: string, kind: Kind) => string {
  if (colour === null || colour === VIOLET) return (purple) => purple;
  const turn = turnOf(colour);
  return (purple, kind) => turned(purple, kind, 'light', turn);
}

/// Why `colour` is not taken as the accent, or none where it is: too grey, or a tone it gives too near the colour of a
/// meaning, the nearest named.
export function refusal(colour: string): Refusal | null {
  if (strength(colour) < GREY * strength(VIOLET)) return 'grey';
  const given = tones(colour);
  let nearest: { meaning: Meaning; by: number } | null = null;
  for (const theme of THEMES) {
    for (const tone of Object.values(given[theme])) {
      const h = lchOf(tone).h;
      for (const [meaning, hue] of MEANING_HUES) {
        const by = apart(h, hue);
        if (by < APART && (nearest === null || by < nearest.by)) nearest = { meaning, by };
      }
    }
  }
  return nearest?.meaning ?? null;
}

/// The accents offered beside a colour of the owner's own, each the accent it gives in light, at the purple's strength,
/// from blue to purple, the hues free of the meanings; none of them near a colour of a meaning.
export const PRESETS: { name: string; colour: string }[] = [
  { name: 'Blue', colour: '#1a77f1' },
  { name: 'Indigo', colour: '#616bf6' },
  { name: 'Violet', colour: VIOLET },
  { name: 'Purple', colour: '#b52ff6' },
];

const KEY = 'wf-agents.accent';

const HEX = /^#[0-9a-f]{6}$/;

/// The accent in force, none for the page's own violet; and a colour kept in this browser that is refused now, with why.
export type Accent = { colour: string | null; refused: { colour: string; why: Refusal } | null };

let state: Accent = { colour: null, refused: null };
const listeners = new Set<() => void>();

function kept(): string | null {
  try {
    const v = localStorage.getItem(KEY)?.toLowerCase() ?? null;
    return v !== null && HEX.test(v) ? v : null;
  } catch {
    return null;
  }
}

function keep(colour: string | null) {
  try {
    if (colour === null) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, colour);
  } catch {
    // Kept only where the browser lets it be: the page is drawn in it until it is left.
  }
}

/// Sets the tones `colour` gives on the root, or takes them away for the page's own violet.
function draw(colour: string | null) {
  const style = document.documentElement.style;
  const given = colour === null ? null : tones(colour);
  for (const theme of THEMES) {
    for (const tone of Object.keys(TONES[theme])) {
      if (given === null) style.removeProperty(ownName(theme, tone));
      else style.setProperty(ownName(theme, tone), given[theme][tone]);
    }
  }
}

function told(next: Accent) {
  state = next;
  for (const l of listeners) l();
}

/// Draws the page in the accent kept in this browser, as the page starts: a kept value that is not a colour is taken as
/// none, as a theme the page does not know is; one refused now is not drawn, and the Settings view says why.
export function startAccent(): void {
  const colour = kept();
  const why = colour === null || colour === VIOLET ? null : refusal(colour);
  const taken = why === null && colour !== VIOLET ? colour : null;
  draw(taken);
  told({ colour: taken, refused: why === null || colour === null ? null : { colour, why } });
}

/// Draws the page in `colour`, the page's own violet for none, and keeps it in this browser; or, where it is refused,
/// leaves the accent as it is and says why.
export function pickAccent(colour: string | null): Refusal | null {
  const c = colour === null ? null : colour.toLowerCase();
  if (c !== null && !HEX.test(c)) throw new Error(`${colour} is not a colour #rrggbb`);
  const taken = c === VIOLET ? null : c;
  const why = taken === null ? null : refusal(taken);
  if (why !== null) return why;
  draw(taken);
  keep(taken);
  told({ colour: taken, refused: null });
  return null;
}

/// The accent in force now.
export const accentNow = (): string | null => state.colour;

/// Tells `listener` each time the accent changes, until the function it gives back is called.
export function onAccent(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/// The accent in force, and a kept one refused now, followed as they change.
export function useAccent(): Accent {
  return useSyncExternalStore(onAccent, () => state);
}
