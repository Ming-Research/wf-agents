import { describe, expect, it, vi } from 'vitest';

import { CHOICE_KEYS } from './kept';
import { hashOf, periodOf, routeOf, tabOf } from './route';

describe('the route', () => {
  it('is the view the hash names, and the console for nothing or a hash the page does not know', () => {
    expect(routeOf('')).toEqual({ view: 'console' });
    expect(routeOf('#/')).toEqual({ view: 'console' });
    expect(routeOf('#/works')).toEqual({ view: 'works' });
    expect(routeOf('#/works/12')).toEqual({ view: 'work', work: 12 });
    expect(routeOf('#/works/12/')).toEqual({ view: 'work', work: 12 });
    expect(routeOf('#/works/twelve')).toEqual({ view: 'console' });
    expect(routeOf('#/works/12/runs/3')).toEqual({ view: 'work', work: 12, run: 3 });
    expect(routeOf('#/works/12/runs/three')).toEqual({ view: 'console' });
    expect(routeOf('#/works/12/runs/3/rounds/2')).toEqual({ view: 'work', work: 12, run: 3, round: 2 });
    expect(routeOf('#/works/12/runs/3/rounds/2/')).toEqual({ view: 'work', work: 12, run: 3, round: 2 });
    expect(routeOf('#/works/12/runs/3/rounds/two')).toEqual({ view: 'console' });
    expect(routeOf('#/works/12/rounds/2')).toEqual({ view: 'console' });
    expect(routeOf('#/console')).toEqual({ view: 'console' });
    expect(routeOf('#/console/items/141')).toEqual({ view: 'console', item: 141 });
    expect(routeOf('#/console/items/one')).toEqual({ view: 'console' });
    expect(routeOf('#/flow')).toEqual({ view: 'flow' });
    expect(routeOf('#/observations')).toEqual({ view: 'observations' });
    expect(routeOf('#/system')).toEqual({ view: 'system' });
    expect(routeOf('#/system/4w')).toEqual({ view: 'system', period: '4w' });
    expect(routeOf('#/system/all')).toEqual({ view: 'system', period: 'all' });
    expect(routeOf('#/system/week')).toEqual({ view: 'system', period: 'week' });
    expect(routeOf('#/system/year')).toEqual({ view: 'console' });
    expect(routeOf('#/settings')).toEqual({ view: 'settings' });
    expect(routeOf('#/somewhere')).toEqual({ view: 'console' });
  });

  it('gives each view its hash, and a work, with a run chosen or none, the tab of the works', () => {
    expect(hashOf({ view: 'work', work: 7 })).toBe('#/works/7');
    expect(hashOf({ view: 'flow' })).toBe('#/flow');
    expect(tabOf({ view: 'work', work: 7 })).toBe('#/works');
    expect(hashOf({ view: 'work', work: 7, run: 3 })).toBe('#/works/7/runs/3');
    expect(tabOf({ view: 'work', work: 7, run: 3 })).toBe('#/works');
    expect(hashOf({ view: 'work', work: 7, run: 3, round: 2 })).toBe('#/works/7/runs/3/rounds/2');
    expect(routeOf(hashOf({ view: 'work', work: 7, run: 3, round: 2 }))).toEqual({
      view: 'work',
      work: 7,
      run: 3,
      round: 2,
    });
    expect(tabOf({ view: 'work', work: 7, run: 3, round: 2 })).toBe('#/works');
    expect(tabOf({ view: 'console' })).toBe('#/console');
    expect(hashOf({ view: 'console', item: 141 })).toBe('#/console/items/141');
    expect(tabOf({ view: 'console', item: 141 })).toBe('#/console');
    expect(hashOf({ view: 'system', period: '4w' })).toBe('#/system/4w');
    expect(hashOf({ view: 'system', period: 'week' })).toBe('#/system/week');
    expect(tabOf({ view: 'system', period: 'all' })).toBe('#/system');
    expect(periodOf({ view: 'system' })).toBe('week');
    expect(periodOf({ view: 'system', period: 'all' })).toBe('all');
    expect(periodOf(routeOf('#/system/week'))).toBe('week');
    expect(routeOf(hashOf({ view: 'system', period: 'week' }))).toEqual({ view: 'system', period: 'week' });
  });

  it('counts the system over the span the owner last picked where the address names none, and over the week where none was', () => {
    const kept = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => kept.get(key) ?? null,
      setItem: (key: string, value: string) => void kept.set(key, value),
      removeItem: (key: string) => void kept.delete(key),
    });
    try {
      expect(periodOf({ view: 'system' })).toBe('week');
      kept.set(CHOICE_KEYS.systemPeriod, '4w');
      expect(periodOf({ view: 'system' })).toBe('4w');
      // The address names its own span, which the one kept does not override.
      expect(periodOf({ view: 'system', period: 'all' })).toBe('all');
      // What is kept that is no span the page has is no choice.
      kept.set(CHOICE_KEYS.systemPeriod, 'a fortnight');
      expect(periodOf({ view: 'system' })).toBe('week');
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
