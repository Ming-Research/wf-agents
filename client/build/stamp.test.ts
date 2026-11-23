import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { build } from 'vite';
import { describe, expect, it } from 'vitest';
import { STAMP, sourceStamp, sourceStampPlugin } from './stamp';

// The SHA-256 of "abc", from FIPS 180-4: the same value crates/web/build_support/stamp.rs is held to, so that the two
// sides hash the same bytes alike.
const ABC = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

function client(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), 'stamp-'));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, '..'), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

describe('the stamp of the page sources', () => {
  it('lists the files the build read with their SHA-256, by their path from client/', () => {
    const root = client({ 'src/a.ts': 'abc', 'src/b/c.tsx': 'abc', 'package-lock.json': 'abc' });
    const stamp = sourceStamp(root, [join(root, 'src/b/c.tsx'), join(root, 'src/a.ts')], [join(root, 'package-lock.json')]);
    expect(stamp).toBe(`${ABC}  package-lock.json\n${ABC}  src/a.ts\n${ABC}  src/b/c.tsx\n`);
  });

  it('leaves out a file the build does not read, whatever else is beside it', () => {
    const root = client({ 'src/a.ts': 'abc', 'src/a.test.ts': 'a test', 'tsconfig.tsbuildinfo': 'cache' });
    expect(sourceStamp(root, [join(root, 'src/a.ts')])).toBe(`${ABC}  src/a.ts\n`);
  });

  it('leaves out a virtual module, node_modules, what is outside client/ and what is no file, and cuts off a query', () => {
    const root = client({ 'src/a.css': 'abc', 'node_modules/x/index.js': 'x' });
    const stamp = sourceStamp(root, [
      '\0vite/preload-helper',
      join(root, 'node_modules/x/index.js'),
      join(root, '../elsewhere.ts'),
      join(root, 'src/none.ts'),
      join(root, 'src'),
      `${join(root, 'src/a.css')}?inline`,
    ]);
    expect(stamp).toBe(`${ABC}  src/a.css\n`);
  });
});

describe('the stamp a build writes', () => {
  it('lists the files the build read, with the page, the lock file and the type configuration, and no other', async () => {
    const root = client({
      'index.html': '<!doctype html><script type="module" src="/src/main.ts"></script>',
      'src/main.ts': "import { a } from './a';\nconsole.log(a);\n",
      'src/a.ts': 'export const a = 1;\n',
      'src/a.test.ts': 'a test, which no build reads',
      'src/unused.ts': 'export const unused = 1;\n',
      'package-lock.json': '{}',
      'tsconfig.json': '{}',
    });
    await build({
      root,
      configFile: false,
      logLevel: 'silent',
      plugins: [sourceStampPlugin()],
      build: { outDir: 'dist', emptyOutDir: true },
    });
    // Each line is a SHA-256 in hex (64 characters), two spaces and the path.
    const lines = readFileSync(join(root, 'dist', STAMP), 'utf8').split('\n').filter((line) => line !== '');
    expect(lines.map((line) => line.slice(66))).toEqual([
      'index.html',
      'package-lock.json',
      'src/a.ts',
      'src/main.ts',
      'tsconfig.json',
    ]);
    expect(lines.every((line) => /^[0-9a-f]{64} {2}/.test(line))).toBe(true);
  });
});
