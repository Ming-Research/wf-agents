import { describe, expect, it } from 'vitest';

import { hashOf, routeOf, tabOf } from './route';

describe('the route', () => {
  it('is the view the hash names, and the list of works for nothing or a hash the page does not know', () => {
    expect(routeOf('')).toEqual({ view: 'works' });
    expect(routeOf('#/')).toEqual({ view: 'works' });
    expect(routeOf('#/works')).toEqual({ view: 'works' });
    expect(routeOf('#/works/12')).toEqual({ view: 'work', work: 12 });
    expect(routeOf('#/works/12/')).toEqual({ view: 'work', work: 12 });
    expect(routeOf('#/works/twelve')).toEqual({ view: 'works' });
    expect(routeOf('#/console')).toEqual({ view: 'console' });
    expect(routeOf('#/flow')).toEqual({ view: 'flow' });
    expect(routeOf('#/somewhere')).toEqual({ view: 'works' });
  });

  it('gives each view its hash, and a work the tab of the works', () => {
    expect(hashOf({ view: 'work', work: 7 })).toBe('#/works/7');
    expect(hashOf({ view: 'flow' })).toBe('#/flow');
    expect(tabOf({ view: 'work', work: 7 })).toBe('#/works');
    expect(tabOf({ view: 'console' })).toBe('#/console');
  });
});
