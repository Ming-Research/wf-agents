// How the page measures and makes a colour: not built yet, so that the accent's tests fail on what they hold.

export type Lch = { l: number; c: number; h: number };

export const labOf = (hex: string): [number, number, number] => [hex.length, 0, 0];
export const lchOf = (hex: string): Lch => ({ l: hex.length, c: 0, h: 0 });
export const widest = (l: number, h: number): number => l + h;
export const hexOf = (l: number, c: number, h: number): string => `#${[l, c, h].join('')}`;
export const contrast = (a: string, b: string): number => (a === b ? 1 : 2);
export const apart = (a: number, b: number): number => a - b;
