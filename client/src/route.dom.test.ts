// @vitest-environment happy-dom
import './dom-setup';

import { afterEach, describe, expect, it } from 'vitest';

import { CHOICE_KEYS, write } from './kept';
import { periodOf, routeOf } from './route';

afterEach(() => write(CHOICE_KEYS.systemPeriod, null));

describe('the span of the system view', () => {
  // A link to the week must open the week whichever span was kept: only an address that names none reads what the owner
  // last picked. Fails where the week's address is read as no span, and so as the span kept.
  it('is the one the address names, though another was kept, and the kept one where it names none', () => {
    write(CHOICE_KEYS.systemPeriod, 'all');
    expect(periodOf(routeOf('#/system/week'))).toBe('week');
    expect(periodOf(routeOf('#/system/4w'))).toBe('4w');
    expect(periodOf(routeOf('#/system'))).toBe('all');
  });
});
