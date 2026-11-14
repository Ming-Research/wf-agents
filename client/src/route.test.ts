import { describe, expect, it } from 'vitest';

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
    expect(routeOf('#/console')).toEqual({ view: 'console' });
    expect(routeOf('#/flow')).toEqual({ view: 'flow' });
    expect(routeOf('#/observations')).toEqual({ view: 'observations' });
    expect(routeOf('#/system')).toEqual({ view: 'system' });
    expect(routeOf('#/system/4w')).toEqual({ view: 'system', period: '4w' });
    expect(routeOf('#/system/all')).toEqual({ view: 'system', period: 'all' });
    expect(routeOf('#/system/week')).toEqual({ view: 'console' });
    expect(routeOf('#/settings')).toEqual({ view: 'settings' });
    expect(routeOf('#/somewhere')).toEqual({ view: 'console' });
  });

  it('gives each view its hash, and a work, with a run chosen or none, the tab of the works', () => {
    expect(hashOf({ view: 'work', work: 7 })).toBe('#/works/7');
    expect(hashOf({ view: 'flow' })).toBe('#/flow');
    expect(tabOf({ view: 'work', work: 7 })).toBe('#/works');
    expect(hashOf({ view: 'work', work: 7, run: 3 })).toBe('#/works/7/runs/3');
    expect(tabOf({ view: 'work', work: 7, run: 3 })).toBe('#/works');
    expect(tabOf({ view: 'console' })).toBe('#/console');
    expect(hashOf({ view: 'system', period: '4w' })).toBe('#/system/4w');
    expect(hashOf({ view: 'system', period: 'week' })).toBe('#/system');
    expect(tabOf({ view: 'system', period: 'all' })).toBe('#/system');
    expect(periodOf({ view: 'system' })).toBe('week');
    expect(periodOf({ view: 'system', period: 'all' })).toBe('all');
  });
});
