// The list vitest writes into the file it is given (`vitest list --json=<path>`), as the fast workflow reads it to find
// the page's tests a name asks for (`.github/workflows/fast.yml`, the step that finds the page tests named): each test
// with the name its results give it (its describes and its title joined by ` > `) and its file, from the real vitest, so
// that a version of it that changes the file's form or the option's meaning fails here and not in a run that names a
// test. About 1 to 2 s alone: one process of vitest, with no browser project.

import { execFile } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { afterEach, describe, expect, it } from 'vitest';

const client = fileURLToPath(new URL('..', import.meta.url));
let dir = '';
afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe('vitest list', () => {
  it('writes its list into the file it is given, each test with the name its results give and its file', async () => {
    // A project of one test in a directory of its own, which finds `vitest` through a link to the page's.
    dir = mkdtempSync(join(tmpdir(), 'wf-list-'));
    symlinkSync(join(client, 'node_modules'), join(dir, 'node_modules'), 'dir');
    const file = join(dir, 'a.test.ts');
    writeFileSync(file, "import { describe, it } from 'vitest';\ndescribe('a describe', () => it('its title', () => undefined));\n");
    const out = join(dir, 'list.json');
    // The one run of vitest this is is not the environment of the one it starts.
    const env = Object.fromEntries(
      Object.entries(process.env).filter(([k]) => !/^(VITEST|TEST$|NODE_ENV)/.test(k)),
    );
    await promisify(execFile)('npx', ['--no-install', 'vitest', 'list', '--root', dir, `--json=${out}`], {
      cwd: client,
      env,
    });
    const listed = JSON.parse(readFileSync(out, 'utf8')) as { name: string; file: string }[];
    expect(listed.map((t) => ({ name: t.name, file: t.file }))).toEqual([{ name: 'a describe > its title', file }]);
  });
});
