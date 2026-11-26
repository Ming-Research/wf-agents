// The look of the page's two dialogs in a real browser (Chromium), where the styles apply: the question before a command
// (`Confirm`) and the window a diagram opens larger in (`DiagramWindow`). They keep a look of their own, the exception
// to the page's sizes, heights and corners (decision the-dialogs-keep-a-look-of-their-own): 14 px type, buttons 32 px
// high with 6 px corners, panels with 12 px corners and a shadow of their own, in the light and the dark theme. Each is
// read against the named value it is to take (`--dialog-*` in tokens.css), and the named values against what they were,
// so that a size written out in their styles again, or a token that drifts, which no other test sees, fails here.

import './styles/tokens.css';
import './styles/parts.css';

import type { ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { DiagramWindow } from './diagram';
import { Confirm } from './parts';

/// What a named value of the page comes to as a style, as the browser computes it in the theme now in use: a probe
/// takes the property from the value, so that the test does not write the value out again.
function named(property: string, value: string): string {
  const probe = document.createElement('div');
  probe.style.setProperty(property, value);
  document.body.append(probe);
  const computed = getComputedStyle(probe).getPropertyValue(property);
  probe.remove();
  return computed;
}

const DRAWN = { svg: '<svg viewBox="0 0 120 40" width="120" height="40"></svg>', width: 120, height: 40 };

describe.each(['light', 'dark'] as const)('the page’s dialogs in the %s theme', (theme) => {
  // What a test drew, taken down after it; a test that draws nothing leaves both unset.
  let drawn: { host: HTMLDivElement; root: Root } | undefined;
  afterEach(() => {
    drawn?.root.unmount();
    drawn?.host.remove();
    drawn = undefined;
    document.documentElement.removeAttribute('data-theme');
  });

  function draw(dialog: ReactNode, selector: string): HTMLDialogElement {
    document.documentElement.setAttribute('data-theme', theme);
    const host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    drawn = { host, root };
    flushSync(() => root.render(dialog));
    return host.querySelector(selector) as HTMLDialogElement;
  }

  // The values the look had before it was given names, which the tokens must still hold.
  it('names the dialogs’ own look in the values it had', () => {
    const root = getComputedStyle(document.documentElement);
    const token = (name: string) => root.getPropertyValue(name).trim();
    expect.soft(token('--dialog-size')).toBe('14px');
    expect.soft(token('--dialog-line-height')).toBe('1.5');
    expect.soft(token('--dialog-button')).toBe('32px');
    expect.soft(token('--dialog-button-corner')).toBe('6px');
    expect.soft(token('--dialog-corner')).toBe('12px');
    expect.soft(token('--dialog-shadow')).toMatch(/^0 0 0 1px .+, 0 8px 16px .+, 0 24px 48px .+$/);
  });

  // The parts of a look are asserted softly, so that a run says every part that is wrong and not only the first.
  it('draws the question before a command in its own size, with its own corners and shadow and buttons of its own height and corner', async () => {
    const dialog = draw(
      <Confirm title="Cancel this work?" verb="Cancel work" onAnswer={() => undefined}>
        It stops, and its branch is let go.
      </Confirm>,
      'dialog.confirm',
    );
    await expect.poll(() => dialog.open).toBe(true);
    const size = named('font-size', 'var(--dialog-size)');
    const look = getComputedStyle(dialog);
    expect.soft(look.fontSize).toBe(named('font-size', 'var(--size-body)'));
    expect.soft(look.borderRadius).toBe(named('border-radius', 'var(--dialog-corner)'));
    expect.soft(look.boxShadow).toBe(named('box-shadow', 'var(--dialog-shadow)'));
    // The shadow's first layer is the page's line in this theme: the token takes `--line` where it is set on the root.
    expect.soft(look.boxShadow.startsWith(named('color', 'var(--line)'))).toBe(true);
    expect.soft(getComputedStyle(dialog.querySelector('.confirm-body') as HTMLElement).fontSize).toBe(size);
    const buttons = [...dialog.querySelectorAll<HTMLButtonElement>('.confirm-buttons .button')];
    expect(buttons).toHaveLength(2);
    for (const button of buttons) {
      const ours = getComputedStyle(button);
      expect.soft(ours.fontSize).toBe(size);
      expect.soft(ours.height).toBe(named('height', 'var(--dialog-button)'));
      expect.soft(ours.borderRadius).toBe(named('border-radius', 'var(--dialog-button-corner)'));
    }
  });

  it('draws the window a diagram opens larger in, in the body size, with the dialogs’ own corners and shadow', async () => {
    const dialog = draw(
      <DiagramWindow drawn={DRAWN} id="wf-diagram-test" onClose={() => undefined} />,
      'dialog.diagram-window',
    );
    await expect.poll(() => dialog.open).toBe(true);
    const look = getComputedStyle(dialog);
    expect.soft(look.fontSize).toBe(named('font-size', 'var(--size-body)'));
    expect.soft(look.borderRadius).toBe(named('border-radius', 'var(--dialog-corner)'));
    expect.soft(look.boxShadow).toBe(named('box-shadow', 'var(--dialog-shadow)'));
  });
});
