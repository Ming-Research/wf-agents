// How the page measures and makes a colour, for the accent (accent.ts): a colour of sRGB as `#rrggbb`, its lightness,
// chroma and hue in OKLCH, the most chroma sRGB holds at a lightness and a hue, and how far two colours stand apart
// for reading, by WCAG's contrast. OKLCH is the polar form of OKLab (Björn Ottosson, 2020), whose lightness is the
// same for every hue, so that a tone turned to another hue at the same lightness reads about as light as before.

/// A colour's lightness (0 to 1), chroma and hue (degrees) in OKLCH.
export type Lch = { l: number; c: number; h: number };

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

/// The channels of `#rrggbb`, each from 0 to 1, linear.
function linear(hex: string): [number, number, number] {
  const at = (i: number) => toLinear(Number.parseInt(hex.slice(1 + 2 * i, 3 + 2 * i), 16) / 255);
  return [at(0), at(1), at(2)];
}

/// A colour in OKLab: its lightness and its two axes.
export function labOf(hex: string): [number, number, number] {
  const [r, g, b] = linear(hex);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

export function lchOf(hex: string): Lch {
  const [l, a, b] = labOf(hex);
  return { l, c: Math.hypot(a, b), h: ((Math.atan2(b, a) * 180) / Math.PI + 360) % 360 };
}

/// The linear sRGB channels of a colour of OKLCH, each outside 0 to 1 where sRGB does not hold it.
function rgbOf(l: number, c: number, h: number): [number, number, number] {
  const a = c * Math.cos((h * Math.PI) / 180);
  const b = c * Math.sin((h * Math.PI) / 180);
  const l3 = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m3 = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s3 = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
    -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
    -0.0041960863 * l3 - 0.7034186147 * m3 + 1.707614701 * s3,
  ];
}

/// How far a channel may stand outside 0 to 1 and still be taken as in sRGB: what rounding to eight bits does not see.
const SLACK = 1e-4;

const holds = (rgb: number[]) => rgb.every((x) => x >= -SLACK && x <= 1 + SLACK);

/// The most chroma sRGB holds at lightness `l` and hue `h`.
export function widest(l: number, h: number): number {
  let low = 0;
  let high = 0.5;
  for (let i = 0; i < 32; i += 1) {
    const middle = (low + high) / 2;
    if (holds(rgbOf(l, middle, h))) low = middle;
    else high = middle;
  }
  return low;
}

/// The colour of lightness `l`, chroma `c` and hue `h` as `#rrggbb`, its chroma lowered, at the same lightness and hue,
/// to the most sRGB holds there.
export function hexOf(l: number, c: number, h: number): string {
  const rgb = rgbOf(l, Math.min(c, widest(l, h)), h);
  return `#${rgb
    .map((x) =>
      Math.round(Math.min(1, Math.max(0, toGamma(x))) * 255)
        .toString(16)
        .padStart(2, '0'),
    )
    .join('')}`;
}

/// How light a colour is to the eye, by WCAG: its relative luminance.
function luminance(hex: string): number {
  const [r, g, b] = linear(hex);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/// The contrast of two colours by WCAG, from 1 (none) to 21 (black on white).
export function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/// How many degrees two hues stand apart, the shorter way round: from 0 to 180.
export function apart(a: number, b: number): number {
  const d = Math.abs(a - b) % 360;
  return Math.min(d, 360 - d);
}
