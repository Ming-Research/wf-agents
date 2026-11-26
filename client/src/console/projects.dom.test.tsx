// @vitest-environment happy-dom
import '../dom-setup';

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it } from 'vitest';

import { ProjectList } from './projects';

afterEach(() => {
  document.body.innerHTML = '';
});

describe("the console's list of projects", () => {
  /// The counts drawn after each project's name, in order, each by its colour and number.
  function drawn(counts: Map<string, { waiting: number; ahead: number; unread: number }>): Record<string, string[]> {
    const container = document.body.appendChild(document.createElement('div'));
    const root = createRoot(container);
    const projects = [...counts.keys()];
    act(() =>
      root.render(<ProjectList projects={projects} counts={counts} shown={projects[0]} onShow={() => {}} onOrder={() => {}} />),
    );
    const of: Record<string, string[]> = {};
    for (const row of container.querySelectorAll('.console-project')) {
      const name = row.querySelector('.console-project-title')?.textContent ?? '';
      of[name] = [...row.querySelectorAll('.bar-count')].map(
        (c) => `${c.classList.contains('unread') ? 'accent' : c.classList.contains('ahead') ? 'orange' : 'red'} ${c.textContent}`,
      );
    }
    act(() => root.unmount());
    container.remove();
    return of;
  }

  it("shows each project's replies not read, questions to answer ahead and waits, as the bar does beside Console", () => {
    const of = drawn(
      new Map([
        ['wf-agents', { waiting: 4, ahead: 2, unread: 30 }],
        ['Whitefoot', { waiting: 0, ahead: 0, unread: 12 }],
        ['Halo-wf', { waiting: 1, ahead: 0, unread: 0 }],
        ['Snowghost-wf', { waiting: 0, ahead: 0, unread: 0 }],
      ]),
    );
    expect(of['wf-agents']).toEqual(['accent 30', 'orange 2', 'red 4']);
    expect(of['Whitefoot']).toEqual(['accent 12']);
    expect(of['Halo-wf']).toEqual(['red 1']);
    expect(of['Snowghost-wf']).toEqual([]);
  });
});
