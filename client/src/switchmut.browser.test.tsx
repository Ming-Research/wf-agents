// One-off demonstration, not kept: the switch test of the work, with a style that breaks what it protects.

import './styles/tokens.css';
import './styles/parts.css';

import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';
import { userEvent } from 'vitest/browser';

import { Switch } from './parts';

describe('a broken switch', () => {
  let host: HTMLDivElement;
  let style: HTMLStyleElement;
  afterEach(() => {
    host.remove();
    style.remove();
  });

  const run = async (broken: string) => {
    style = document.createElement('style');
    style.textContent = broken;
    document.head.append(style);
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
    const look = (b: HTMLElement) => ({ cursor: getComputedStyle(b).cursor, ground: getComputedStyle(b).backgroundColor });
    const still = { taken: look(taken), other: look(other) };
    await userEvent.hover(other);
    await expect.poll(() => look(other).ground).not.toBe(still.other.ground);
    expect(look(other).cursor).toBe('pointer');
    await userEvent.hover(taken);
    await expect.poll(() => look(other).ground).toBe(still.other.ground);
    expect(look(taken)).toEqual({ cursor: 'default', ground: still.taken.ground });
    root.unmount();
  };

  it('with a tint on the choice taken under the pointer', () => run(".switch-choice[aria-pressed='true']:hover { background: rgb(255, 0, 0); }"));
  it('with the hand on the choice taken', () => run(".switch-choice[aria-pressed='true'] { cursor: pointer; }"));
  it('with nothing broken', () => run(''));
});
