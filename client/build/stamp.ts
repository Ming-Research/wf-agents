// The stamp of the page's sources: `dist/source.sha256`, which crates/web/build_support/stamp.rs reads again when the
// program is built. A line for each file the build read outside `node_modules` (the page's sources, its configuration,
// the lock file, the entry page, the files copied as they are), the SHA-256 of its bytes in hex, two spaces, its path
// from `client/` with `/` between the parts. A file the build does not read, such as a test or the type checker's
// cache, is not in it, so that changing one does not make the page look out of date.
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { isAbsolute, relative, resolve, sep } from 'node:path';
import type { Plugin } from 'vite';

/** The name of the stamp in the page's build directory; the build script of `web` keeps it out of the page's files. */
export const STAMP = 'source.sha256';

/**
 * The lines of the stamp for the files that `ids` (the build's module ids) and `more` (files the build reads that are
 * no module) name: a virtual module (an id that begins with a NUL), a file in `node_modules`, a file outside `root`
 * and an id that names no file are left out, and a query on an id is cut off.
 */
export function sourceStamp(root: string, ids: Iterable<string>, more: Iterable<string> = []): string {
  const paths = new Set<string>();
  for (const id of [...ids, ...more]) {
    if (id.startsWith('\0')) continue;
    const absolute = resolve(root, id.split('?')[0]);
    const path = relative(root, absolute);
    if (path === '' || path.startsWith('..') || isAbsolute(path)) continue;
    const parts = path.split(sep);
    if (parts.includes('node_modules')) continue;
    if (!existsSync(absolute) || !statSync(absolute).isFile()) continue;
    paths.add(parts.join('/'));
  }
  return [...paths]
    .sort()
    .map((path) => `${createHash('sha256').update(readFileSync(resolve(root, path))).digest('hex')}  ${path}\n`)
    .join('');
}

/** Every file under `dir`, by its path. */
function filesUnder(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? filesUnder(resolve(dir, entry.name)) : [resolve(dir, entry.name)],
  );
}

/** Writes the stamp into the build directory when a build ends. */
export function sourceStampPlugin(): Plugin {
  const ids = new Set<string>();
  let root = '';
  let out = '';
  let more: string[] = [];
  return {
    name: 'wf-agents-source-stamp',
    apply: 'build',
    configResolved(config) {
      root = config.root;
      out = resolve(config.root, config.build.outDir);
      more = ['index.html', 'tsconfig.json', 'package-lock.json'].map((name) => resolve(config.root, name));
      if (config.configFile) more.push(config.configFile);
      if (config.publicDir && existsSync(config.publicDir)) more.push(...filesUnder(config.publicDir));
    },
    moduleParsed(info) {
      ids.add(info.id);
    },
    closeBundle() {
      writeFileSync(resolve(out, STAMP), sourceStamp(root, ids, more));
    },
  };
}
