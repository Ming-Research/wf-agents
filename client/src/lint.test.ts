// What the page's lint keeps out of its parts: a read made by hand, and a box of words that is not a draft. What events
// keep current is read through the reader (`useResource`, `useShared`, `useResources`, over `reader.ts`), one read at a
// time for each path; `getJson` and `getBody` read at once, with nothing shared between the parts that show a path. The
// words the owner types are a draft (`useDraft`), which keeps them when the box goes away and comes back: Primer's
// `Textarea` and `TextInput`, and the HTML `input` and `textarea`, are drawn in `src/parts.tsx` alone, as parts that take
// a draft. The test lints sources with the page's own configuration, as `npm run lint` does.

import { Linter } from 'eslint';
import { describe, expect, it } from 'vitest';

// @ts-expect-error the configuration is plain JavaScript and has no declarations
import config from '../eslint.config.js';

/// The messages of the rule `ruleId` that linting `code` as the file `filename` gives.
function flagged(ruleId: string, filename: string, code: string): string[] {
  return new Linter({ configType: 'flat' })
    .verify(code, config as Linter.Config[], { filename })
    .filter((m) => m.ruleId === ruleId)
    .map((m) => m.message);
}

/// The messages of the import rule that linting `code` as the file `filename` gives.
const restricted = (filename: string, code: string): string[] => flagged('no-restricted-imports', filename, code);

/// The messages of the rule that keeps the HTML boxes out that linting `code` as the file `filename` gives.
const elements = (filename: string, code: string): string[] => flagged('no-restricted-syntax', filename, code);

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

describe('a box the owner types in', () => {
  const primer = (names: string) => `import { ${names} } from '@primer/react';\nexport const x = [${names.replace(/ as \w+/g, '')}];\n`;

  it("is refused as Primer's Textarea or TextInput in a part of the page, wherever the part is", () => {
    expect(restricted('src/pages/Probe.tsx', primer('Textarea'))).toHaveLength(1);
    expect(restricted('src/pages/Probe.tsx', primer('TextInput'))).toHaveLength(1);
    expect(restricted('src/Probe.tsx', primer('Button, Textarea'))).toHaveLength(1);
    expect(restricted('src/item/Probe.tsx', primer('TextInput'))).toHaveLength(1);
    expect(restricted('src/pages/deep/Probe.tsx', primer('Textarea'))).toHaveLength(1);
  });

  it('is refused under another name or as the whole package, and where the reads are allowed', () => {
    expect(restricted('src/pages/Probe.tsx', primer('Textarea as Box'))).toHaveLength(1);
    expect(restricted('src/pages/Probe.tsx', "import * as Primer from '@primer/react';\nexport const x = Primer;\n")).toHaveLength(1);
    expect(restricted('src/pages/Probe.tsx', "export { Textarea } from '@primer/react';\n")).toHaveLength(1);
    expect(restricted('src/reader.ts', primer('Textarea'))).toHaveLength(1);
    expect(restricted('src/pages/RoundView.tsx', primer('TextInput'))).toHaveLength(1);
  });

  it('is refused as the HTML input or textarea, in every part but the one that draws the boxes', () => {
    expect(elements('src/pages/Probe.tsx', 'export const x = <textarea />;\n')).toHaveLength(1);
    expect(elements('src/pages/Probe.tsx', 'export const x = <input type="text" />;\n')).toHaveLength(1);
    expect(elements('src/Probe.tsx', 'export const x = <div><textarea value="" onChange={() => {}} /></div>;\n')).toHaveLength(1);
    expect(elements('src/parts.tsx', 'export const x = <input type="file" />;\n')).toEqual([]);
  });

  it('is allowed in the one file of the parts that take a draft, which still reads nothing by hand', () => {
    expect(restricted('src/parts.tsx', primer('Textarea, TextInput'))).toEqual([]);
    expect(restricted('src/parts.tsx', "import { getJson } from './http';\nexport const x = getJson;\n")).toHaveLength(1);
  });

  it('is not refused for the rest of Primer, nor for other elements', () => {
    expect(restricted('src/pages/Probe.tsx', primer('Button, Select, Checkbox, FormControl'))).toEqual([]);
    expect(elements('src/pages/Probe.tsx', 'export const x = <div><select /><button /></div>;\n')).toEqual([]);
  });
});
