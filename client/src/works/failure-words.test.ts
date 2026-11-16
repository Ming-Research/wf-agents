import { describe, expect, it } from 'vitest';

import type { Outage, Waited } from '../works-api';
import { callWords, failureWords, outageWords, unmetWords, waitedWords } from './words';

const outage = (o: Partial<Outage>): Outage => ({
  service: 'agents',
  failure: 'usageLimit',
  why: "You've hit your limit",
  work: 3,
  run: 2,
  round: 1,
  since: 0,
  checked: null,
  until: { kind: 'checked', at: 0 },
  holds: true,
  ...o,
});

describe('the words of a round that failed and of the waits that follow', () => {
  it('names each kind of cause', () => {
    expect(failureWords('usageLimit')).toBe("the account's usage limit was reached");
    expect(failureWords('passing')).toBe('in passing');
    expect(unmetWords({ kind: 'unreadable', why: 'missing field `matters`' })).toBe(
      'what it returned could not be read: missing field `matters`',
    );
  });

  it("says the engine's wait as a call, with how many failed in a row", () => {
    expect(callWords({ kind: 'wait', until: 0, failed: 3 })).toMatch(/^the engine's wait until .*\(3 in a row\)$/);
  });

  it('says a wait every work shares in three sentences: what is down since when, why, and what it holds', () => {
    const words = outageWords(outage({}));
    expect(words).toMatch(
      /^The account's usage limit was reached since [^.]*\. You've hit your limit\. No agent's round begins until it runs again\.$/,
    );
    expect(words).not.toContain('W3');
    // What the owner does where the command line cannot run is the engine's to say, on the wait's card, once.
    expect(outageWords(outage({ failure: 'cannotRun' }))).not.toContain('install');
    // What the latest check found down is what is said down, with what it found, a sentence of its own.
    const found = 'It is not signed in, as `claude auth status` says.';
    const checked = outage({
      failure: 'unavailable',
      checked: { at: 0, taken: 2, found: { kind: 'down', failure: 'cannotRun', why: found } },
    });
    expect(outageWords(checked)).toMatch(/^The agents' command line cannot run since /);
    expect(outageWords(checked)).toContain(` the check found: ${found} No agent's round begins until it runs again.`);
    const well = outage({
      holds: false,
      until: { kind: 'well', at: 0 },
      checked: { at: 0, taken: 1, found: { kind: 'well' } },
    });
    expect(outageWords(well)).toMatch(/Every work goes on\.$/);
    const host = outage({ service: 'host', failure: 'offline', until: { kind: 'looked', at: 0 } });
    expect(outageWords(host)).toMatch(/No act of the engine's begins until it is reached again\.$/);
  });

  it('says the wait every work shares as what the agents\' rounds wait for', () => {
    const agents = (failure: Outage['failure']): Waited => ({ kind: 'agents', outage: outage({ failure }) });
    expect(waitedWords(agents('offline'))).toBe("the agents' rounds to begin again, as the network cannot be reached");
  });
});
