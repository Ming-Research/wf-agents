import { describe, expect, it } from 'vitest';

import { askServes, opensOnFlow } from './serves';

describe('opening on the flow', () => {
  it('is for a server that gives the flow alone, where the address names no view', () => {
    for (const hash of ['', '#', '#/', '#/nothing-known']) expect(opensOnFlow('flow', hash)).toBe(true);
  });

  it('is not where the address names a view, which then says what the server lacks', () => {
    for (const hash of ['#/works', '#/works/43', '#/console', '#/flow']) expect(opensOnFlow('flow', hash)).toBe(false);
  });

  it('is not for a server that gives every view, nor while the server is being asked', () => {
    expect(opensOnFlow('all', '')).toBe(false);
    expect(opensOnFlow(null, '')).toBe(false);
  });
});

describe('what the server gives', () => {
  it('is the flow alone when the server has no path for the items', async () => {
    expect(await askServes('', async () => ({ status: 404 }))).toBe('flow');
  });

  it('is every view when the server answers for the items, whatever it answers', async () => {
    expect(await askServes('', async () => ({ status: 200 }))).toBe('all');
    expect(await askServes('', async () => ({ status: 500 }))).toBe('all');
  });

  it('is every view when the server cannot be asked, so that the views say what failed', async () => {
    const down = async () => {
      throw new Error('no answer');
    };
    expect(await askServes('', down)).toBe('all');
  });

  it('is every view with the fixtures, and the server is not asked', async () => {
    let asked = 0;
    const count = async () => {
      asked += 1;
      return { status: 404 };
    };
    expect(await askServes('?fixtures', count)).toBe('all');
    expect(asked).toBe(0);
  });
});
