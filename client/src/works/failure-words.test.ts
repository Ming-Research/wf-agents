import { describe, expect, it } from 'vitest';

import type { Outage, Waited } from '../works-api';
import { callWords, failureWords, outageWords, unmetWords, waitedWords, waitsForOwner } from './words';

const outage = (o: Partial<Outage>): Outage => ({
  service: 'agents',
  failure: 'usageLimit',
  why: "You've hit your limit",
  work: 3,
  run: 2,
  round: 1,
  since: 0,
  until: { kind: 'at', at: 0 },
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

  it('says a wait every work shares with its cause, its end and the round that met it', () => {
    const words = outageWords(outage({}));
    expect(words).toMatch(/^the account's usage limit was reached; no agent's round of any work begins, until /);
    expect(words).toContain("(work 3, round 1 of run 2: You've hit your limit)");
    expect(outageWords(outage({ failure: 'cannotRun', until: { kind: 'tried' } }))).toContain(
      'sign the command line in again or install it; no agent',
    );
    expect(
      outageWords(outage({ failure: 'offline', until: { kind: 'trial', at: 0, look: true } })),
    ).toContain('one round tries once the network is reached');
    const host = outage({ service: 'host', failure: 'offline', until: { kind: 'looked', at: 0 } });
    expect(outageWords(host)).toContain("no act of the engine's of any work begins, once the host is reached");
  });

  it('waits for the owner only where the command line cannot run', () => {
    const agents = (failure: Outage['failure']): Waited => ({ kind: 'agents', outage: outage({ failure }) });
    expect(waitsForOwner(agents('cannotRun'))).toBe(true);
    expect(waitsForOwner(agents('usageLimit'))).toBe(false);
    expect(waitedWords(agents('offline'))).toMatch(/^the agents' rounds to begin again: the network cannot be reached/);
  });
});
