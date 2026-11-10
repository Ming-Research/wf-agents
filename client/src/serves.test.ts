import { describe, expect, it } from 'vitest';

import { askServes } from './serves';

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
