// The look of the page's two dialogs in a real browser (Chromium), where the styles apply: the question before a command
// (`Confirm`) and the window a diagram opens larger in (`DiagramWindow`). Both are drawn in the page's own sizes
// (principle few-sizes-two-weights), with square panels and 3 px buttons (decision the-look-stays-square-and-cool) and
// the shadow of the window that floats over a view, in the light and the dark theme. Each is read against the named
// value it is to take, so that a size written out in their styles again, which no other test sees, fails here.

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
  let host: HTMLDivElement;
  let root: Root;
  afterEach(() => {
    root.unmount();
    host.remove();
    document.documentElement.removeAttribute('data-theme');
  });

  function draw(dialog: ReactNode, selector: string): HTMLDialogElement {
    document.documentElement.setAttribute('data-theme', theme);
    host = document.createElement('div');
    document.body.append(host);
    root = createRoot(host);
    flushSync(() => root.render(dialog));
    return host.querySelector(selector) as HTMLDialogElement;
  }

  // The parts of a look are asserted softly, so that a run says every part that is wrong and not only the first.
  it('draws the question before a command in the body size, with square corners, the window shadow and buttons of the control height and corner', async () => {
    const dialog = draw(
      <Confirm title="Cancel this work?" verb="Cancel work" onAnswer={() => undefined}>
        It stops, and its branch is let go.
      </Confirm>,
      'dialog.confirm',
    );
    await expect.poll(() => dialog.open).toBe(true);
    const body = named('font-size', 'var(--size-body)');
    const look = getComputedStyle(dialog);
    expect.soft(look.fontSize).toBe(body);
    expect.soft(look.borderRadius).toBe('0px');
    expect.soft(look.boxShadow).toBe(named('box-shadow', 'var(--window-shadow)'));
    expect.soft(getComputedStyle(dialog.querySelector('.confirm-body') as HTMLElement).fontSize).toBe(body);
    const buttons = [...dialog.querySelectorAll<HTMLButtonElement>('.confirm-buttons .button')];
    expect(buttons).toHaveLength(2);
    for (const button of buttons) {
      const ours = getComputedStyle(button);
      expect.soft(ours.fontSize).toBe(body);
      expect.soft(ours.height).toBe(named('height', 'var(--control)'));
      expect.soft(ours.borderRadius).toBe(named('border-radius', 'var(--corner)'));
    }
  });

  it('draws the window a diagram opens larger in, in the body size, with square corners and the window shadow', async () => {
    const dialog = draw(
      <DiagramWindow drawn={DRAWN} id="wf-diagram-test" onClose={() => undefined} />,
      'dialog.diagram-window',
    );
    await expect.poll(() => dialog.open).toBe(true);
    const look = getComputedStyle(dialog);
    expect.soft(look.fontSize).toBe(named('font-size', 'var(--size-body)'));
    expect.soft(look.borderRadius).toBe('0px');
    expect.soft(look.boxShadow).toBe(named('box-shadow', 'var(--window-shadow)'));
  });
});
