// The page's segmented switch under the pointer, in a real browser (Chromium): the choice taken does nothing when
// pressed, so it neither takes the hover's tint nor shows the hand; the other choices do both.

import './styles/tokens.css';
import './styles/parts.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { userEvent } from 'vitest/browser';

import { Switch } from './parts';

describe('a segmented switch', () => {
  let host: HTMLDivElement;
  afterEach(() => host.remove());

  it('looks pressable on hover only in the choices not taken', async () => {
    host = document.createElement('div');
    document.body.append(host);
    const root = createRoot(host);
    flushSync(() =>
      root.render(
        <Switch
          label="Whether the work goes on"
          value="on"
          onChange={() => undefined}
          choices={[
            { value: 'on', name: 'Start' },
            { value: 'paused', name: 'Pause' },
          ]}
        />,
      ),
    );
    const [taken, other] = [...host.querySelectorAll<HTMLButtonElement>('.switch-choice')];
    expect(taken.getAttribute('aria-pressed')).toBe('true');
    const look = (b: HTMLElement) => ({ cursor: getComputedStyle(b).cursor, ground: getComputedStyle(b).backgroundColor });
    const still = { taken: look(taken), other: look(other) };

    // The hover reaches the other choice, and its tint comes: waited for as long as the browser tests wait on a
    // condition, not for a time that a busy machine outlasts. The hand is the style's own and does not wait.
    await userEvent.hover(other);
    await expect.poll(() => look(other).ground).not.toBe(still.other.ground);
    expect(look(other).cursor).toBe('pointer');

    // The pointer moves to the choice taken. The other choice has its first look again once the hover has left it,
    // which says the hover has reached this one; a tint it would take starts in the same step, so it is read after.
    await userEvent.hover(taken);
    await expect.poll(() => look(other).ground).toBe(still.other.ground);
    expect(look(taken)).toEqual({ cursor: 'default', ground: still.taken.ground });
    root.unmount();
  });
});
