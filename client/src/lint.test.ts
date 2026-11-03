// What the page's lint keeps out of its parts: a read made by hand. What events keep current is read through the reader
// (`useResource`, `useShared`, `useResources`, over `reader.ts`), one read at a time for each path; `getJson` and
// `getBody` read at once, with nothing shared between the parts that show a path. The test lints two sources with the
// page's own configuration, as `npm run lint` does.

import { Linter } from 'eslint';
import { describe, expect, it } from 'vitest';

// @ts-expect-error the configuration is plain JavaScript and has no declarations
import config from '../eslint.config.js';

/// The messages of the import rule that linting `code` as the file `filename` gives.
function restricted(filename: string, code: string): string[] {
  return new Linter({ configType: 'flat' })
    .verify(code, config as Linter.Config[], { filename })
    .filter((m) => m.ruleId === 'no-restricted-imports')
    .map((m) => m.message);
}

describe('the import of a raw read', () => {
  it('is refused in a part of the page, for getJson and getBody, wherever the part is', () => {
    expect(restricted('src/pages/Probe.tsx', "import { getJson } from '../http';\nexport const x = getJson;\n")).toHaveLength(1);
    expect(restricted('src/Probe.ts', "import { getBody } from './http';\nexport const x = getBody;\n")).toHaveLength(1);
    expect(restricted('src/item/Probe.tsx', "import { getJson } from '../http';\nexport const x = getJson;\n")).toHaveLength(1);
  });

  it('is refused under another name or as the whole module, and from a part nested deeper', () => {
    expect(restricted('src/pages/Probe.tsx', "import { getJson as read } from '../http';\nexport const x = read;\n")).toHaveLength(1);
    expect(restricted('src/pages/Probe.tsx', "import * as http from '../http';\nexport const x = http;\n")).toHaveLength(1);
    expect(restricted('src/pages/deep/Probe.tsx', "import { getBody } from '../../http';\nexport const x = getBody;\n")).toHaveLength(1);
  });

  it('is allowed where the owner opens or scrolls to read: the rounds and the round panel', () => {
    const raw = "import { getJson } from './http';\nexport const x = getJson;\n";
    expect(restricted('src/rounds.ts', raw)).toEqual([]);
    expect(restricted('src/pages/RoundView.tsx', "import { getJson } from '../http';\nexport const x = getJson;\n")).toEqual([]);
  });

  it('is allowed in the reader and the hooks over it, and a read that is no event-driven read stays allowed', () => {
    const raw = "import { getBody } from './http';\nexport const x = getBody;\n";
    expect(restricted('src/reader.ts', raw)).toEqual([]);
    expect(restricted('src/live.ts', raw)).toEqual([]);
    expect(restricted('src/pages/Probe.tsx', "import { getText, postJson } from '../http';\nexport const x = [getText, postJson];\n")).toEqual([]);
  });
});
