// The accent: not built yet, so that its tests fail on what they hold and not on a module missing.

export type Theme = 'light' | 'dark';
export type Kind = 'read' | 'tint';

export const TONES: Record<Theme, Record<string, Kind>> = {
  light: { accent: 'read', 'accent-text': 'read', 'accent-tint': 'tint', head: 'read', 'mark-done': 'read', 'mark-running': 'read' },
  dark: { accent: 'read', 'accent-tint': 'tint', 'mark-done': 'read', 'mark-running': 'read' },
};
export const MEANINGS = { red: ['red'], orange: ['orange'], yellow: ['yellow'], green: ['green'], tools: ['track-tool'] } as const;
export type Meaning = keyof typeof MEANINGS;
export type Refusal = Meaning | 'grey';
export const APART = 45;
export const GREY = 0.4;
export const declared = (opener: string): Record<string, string> => ({ opener });
export const ownName = (theme: Theme, tone: string) => `--own-${tone}-${theme}`;
export const PURPLE: Record<Theme, Record<string, string>> = { light: {}, dark: {} };
export const VIOLET = '#8b5cf6';
export const strength = (colour: string): number => colour.length;
export const tones = (colour: string): Record<Theme, Record<string, string>> => ({ light: { colour }, dark: {} });
export const turning = (colour: string | null) => (purple: string, kind: Kind) => (colour === null ? purple : kind);
export const refusal = (colour: string): Refusal | null => (colour === '' ? 'grey' : null);
export const PRESETS: { name: string; colour: string }[] = [];
export type Accent = { colour: string | null; refused: { colour: string; why: Refusal } | null };
export const startAccent = (): void => undefined;
export const pickAccent = (colour: string | null): Refusal | null => (colour === '' ? 'grey' : null);
export const accentNow = (): string | null => null;
export const useAccent = (): Accent => ({ colour: null, refused: null });
