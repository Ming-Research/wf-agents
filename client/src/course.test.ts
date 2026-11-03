// What the owner's Start sends: the session choice, and the note the owner leaves with the release of a work the
// architect stopped.

import { describe, expect, it } from 'vitest';

import { startBody } from './course';

describe("a Start's request", () => {
  it('has the session choice alone when the owner leaves no note', () => {
    expect(startBody(false, null)).toEqual({ new_session: false });
    expect(startBody(true, '')).toEqual({ new_session: true });
    expect(startBody(false, '  \n ')).toEqual({ new_session: false });
  });

  it('has the note, trimmed, and the owner as its speaker when the owner leaves one', () => {
    expect(startBody(false, ' api.ts is changed. ')).toEqual({
      new_session: false,
      note: 'api.ts is changed.',
      speaker: 'owner',
    });
  });
});
