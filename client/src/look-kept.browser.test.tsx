// What the change of raw values to named values is to leave as it was, read in a real browser (Chromium) where the
// styles apply. A raw value made a named value is the same size, colour or space under a name, so the look of the page
// does not change. The parts the two dialog tests do not read (the dimming under a dialog, its width, its line height,
// the window's size) are read here against the values they had and the tokens they take, and a key cap, which has a
// corner and a line of its own, is read as it was.

import './styles/tokens.css';
import './styles/parts.css';
import './styles/works.css';
import './console/console.css';

import type { ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { DiagramWindow } from './diagram';
import { Confirm } from './parts';

/// What a named value of the page comes to as a style, as the browser computes it in the theme now in use.
function named(property: string, value: string): string {
  const probe = document.createElement('div');
  probe.style.setProperty(property, value);
  document.body.append(probe);
  const computed = getComputedStyle(probe).getPropertyValue(property);
  probe.remove();
  return computed;
}

const DRAWN = { svg: '<svg viewBox="0 0 120 40" width="120" height="40"></svg>', width: 120, height: 40 };

describe.each(['light', 'dark'] as const)('what the named values leave as it was, in the %s theme', (theme) => {
  let host: HTMLDivElement;
  let root: Root;
  afterEach(() => {
    root.unmount();
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  function draw(part: ReactNode): HTMLElement {
    document.documentElement.setAttribute('data-theme', theme);
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    flushSync(() => root.render(part));
    return host;
  }

  it('dims the page under a dialog with the scrim, and gives the question its width and its line height', async () => {
    const dialog = draw(
      <Confirm title="Cancel this work?" verb="Cancel work" onAnswer={() => undefined}>
        It stops, and its branch is let go.
      </Confirm>,
    ).querySelector('dialog.confirm') as HTMLDialogElement;
    await expect.poll(() => dialog.open).toBe(true);
    expect.soft(getComputedStyle(dialog, '::backdrop').backgroundColor).toBe(named('background-color', 'var(--scrim)'));
    expect.soft(getComputedStyle(dialog, '::backdrop').backgroundColor).toBe('rgba(31, 35, 40, 0.35)');
    const room = window.innerWidth - 2 * Number.parseFloat(named('width', 'var(--space-16)'));
    expect.soft(dialog.getBoundingClientRect().width).toBeCloseTo(Math.min(Number.parseFloat(named('width', 'var(--dialog-width)')), room), 3);
    const look = getComputedStyle(dialog);
    const line = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--dialog-line-height'));
    expect.soft(Number.parseFloat(look.lineHeight)).toBeCloseTo(Number.parseFloat(look.fontSize) * line, 3);
  });

  it('keeps the question before a command as main drew it, by the numbers its styles then wrote', async () => {
    // Reference: `parts.css` on main (`git show ff7954d3e:client/src/styles/parts.css`, `.confirm`, `.confirm-body`,
    // `.confirm-buttons .button`, `.diagram-window`): written out here, so that a token that changes with its users
    // does not move the oracle with it.
    const part = draw(
      <>
        <Confirm title="Cancel this work?" verb="Cancel work" onAnswer={() => undefined}>
          It stops, and its branch is let go.
        </Confirm>
        <DiagramWindow drawn={DRAWN} id="wf-diagram-main" onClose={() => undefined} />
      </>,
    );
    const dialog = part.querySelector('dialog.confirm') as HTMLDialogElement;
    const window_ = part.querySelector('dialog.diagram-window') as HTMLDialogElement;
    await expect.poll(() => dialog.open && window_.open).toBe(true);
    const look = getComputedStyle(dialog);
    expect.soft(look.borderTopLeftRadius).toBe('12px');
    expect.soft(look.lineHeight).toBe('19.5px');
    expect.soft(getComputedStyle(window_).borderTopLeftRadius).toBe('12px');
    expect.soft(getComputedStyle(dialog.querySelector('.confirm-body') as HTMLElement).fontSize).toBe('14px');
    for (const button of dialog.querySelectorAll<HTMLButtonElement>('.confirm-buttons .button')) {
      const ours = getComputedStyle(button);
      expect.soft(ours.height).toBe('32px');
      expect.soft(ours.borderTopLeftRadius).toBe('6px');
      expect.soft(ours.fontSize).toBe('14px');
    }
    // Both panels carry the same three shadows: a hairline, then 8px 16px and 24px 48px blurs.
    for (const panel of [dialog, window_]) {
      const shadow = getComputedStyle(panel).boxShadow;
      expect.soft(shadow).toContain('8px 16px');
      expect.soft(shadow).toContain('24px 48px');
      expect.soft(shadow).toContain('rgba(0, 0, 0, 0.08)');
      expect.soft(shadow).toContain('rgba(0, 0, 0, 0.16)');
    }
  });

  it('dims the page under the window a diagram opens larger in, and leaves a margin of four spaces round it', async () => {
    const dialog = draw(<DiagramWindow drawn={DRAWN} id="wf-diagram-kept" onClose={() => undefined} />).querySelector(
      'dialog.diagram-window',
    ) as HTMLDialogElement;
    await expect.poll(() => dialog.open).toBe(true);
    expect.soft(getComputedStyle(dialog, '::backdrop').backgroundColor).toBe(named('background-color', 'var(--scrim)'));
    const margin = 4 * Number.parseFloat(named('width', 'var(--space-16)'));
    const rect = dialog.getBoundingClientRect();
    expect.soft(rect.width).toBeCloseTo(window.innerWidth - margin, 3);
    expect.soft(rect.height).toBeCloseTo(window.innerHeight - margin, 3);
  });

  it('keeps a key cap as it was: a corner of 4 pixels, a line as high as its letters, a cap of 18 pixels', () => {
    const key = draw(<kbd className="key">7</kbd>).querySelector('kbd.key') as HTMLElement;
    const look = getComputedStyle(key);
    expect.soft(look.borderTopLeftRadius).toBe('4px');
    expect.soft(look.lineHeight).toBe(look.fontSize);
    expect.soft(key.getBoundingClientRect().height).toBeCloseTo(18, 3);
    expect.soft(look.borderBottomWidth).toBe('2px');
  });

  it('keeps a size a token writes in em in the em of the part that takes it, not of the root that holds the token', () => {
    const part = draw(
      <div style={{ width: '800px' }}>
        <div className="run-facts" />
        <div className="cw-choice-few" />
      </div>,
    );
    // Reference: CSS Custom Properties Level 1 (an unregistered custom property is a stream of tokens, and an `em` in it
    // is worked out where the property is used, so by the font size of the part that takes it).
    const facts = part.querySelector('.run-facts') as HTMLElement;
    const em = Number.parseFloat(getComputedStyle(facts).fontSize);
    expect(getComputedStyle(facts).gridTemplateColumns).toMatch(new RegExp(`^${6 * em}px ${3 * em}px ${4 * em}px `));
    const folded = part.querySelector('.cw-choice-few') as HTMLElement;
    const line = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--line-height'));
    expect(Number.parseFloat(getComputedStyle(folded).maxHeight)).toBeCloseTo(3 * line * Number.parseFloat(getComputedStyle(folded).fontSize), 3);
  });

  it('keeps the card opaque, which the fade of a folded question is masked by (a mask reads the alpha of its image)', () => {
    draw(<span />);
    // `rgb(` and not `rgba(`: the browser says an alpha under one in the colour's own text.
    expect(named('background-color', 'var(--card)')).toMatch(/^rgb\(/);
  });
});
