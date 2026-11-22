// Probe for the flaky listing of the page's tests (read once, then removed): what `vitest list --project browser --json`
// prints on its standard output, alone and three at once, as the fast workflow runs it. The test fails on purpose, so
// that the result of the run carries what was seen.

import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

type Ran = { code: number | null; out: string; err: string; ms: number };

const client = fileURLToPath(new URL('..', import.meta.url));
const env = Object.fromEntries(
  Object.entries(process.env).filter(([k]) => !/^(VITEST|TEST$|NODE_ENV|VITE)/.test(k)),
);

const list = (project: string) =>
  new Promise<Ran>((done) => {
    const at = Date.now();
    execFile(
      'npx',
      ['--no-install', 'vitest', 'list', '--project', project, '--json'],
      { cwd: client, env, maxBuffer: 1 << 27, timeout: 150_000 },
      (e, out, err) =>
        done({ code: e === null ? 0 : ((e as { code?: number }).code ?? -1), out, err, ms: Date.now() - at }),
    );
  });

const parses = (s: string) => {
  try {
    JSON.parse(s);
    return true;
  } catch {
    return false;
  }
};

const say = (name: string, r: Ran) =>
  `${name}: exit ${r.code}, ${r.ms} ms, stdout ${r.out.length} chars, parses as JSON: ${parses(r.out)}\n` +
  `  stdout begins: ${JSON.stringify(r.out.slice(0, 400))}\n  stderr begins: ${JSON.stringify(r.err.slice(0, 400))}`;

describe('probe', () => {
  it('lists the browser tests alone and three at once', async () => {
    const alone = await list('browser');
    const together = await Promise.all([list('browser'), list('browser'), list('browser')]);
    const unit = await list('unit');
    const seen = [
      say('alone', alone),
      ...together.map((r, k) => say(`one of three at once, ${k + 1}`, r)),
      say('unit project', unit),
    ].join('\n');
    expect(seen).toBe('probe result');
  }, 400_000);
});
