// The page's lint: the recommended JavaScript and TypeScript rules and the rules of hooks, with no warning let
// through (`npm run lint` passes --max-warnings 0); that what events keep current is read through the reader; and that
// every box the owner types in keeps its words as a draft.
import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

/// The reads that are not kept current by events: `getJson` and `getBody` read at once, with no coalescing, no gap
/// between reads and nothing shared between the parts that show a path. Parts read through `useResource`, `useShared`
/// and `useResources`, which the reader (`src/reader.ts`) makes one read at a time for each path.
const RAW_READS = ['getJson', 'getBody'];
const NOT_RAW = 'Read through useResource, useShared or useResources: an event-driven read made by hand is not coalesced.';
const RAW_READ_PATHS = ['./http', '../http', '../../http'].map((name) => ({ name, importNames: RAW_READS, message: NOT_RAW }));

/// The boxes the owner types in: Primer's `Textarea` and `TextInput`, and the HTML elements `input` and `textarea`.
/// Words typed in a box of the page are a draft (`useDraft`, `src/drafts.ts`), which keeps them when the box goes away
/// and comes back and when the page is loaded again. The parts that take a draft (`SayBox`, `SendForm`, `DraftArea`,
/// `DraftInput`, and `FieldInput` for the fields of a form kept as one draft) are in `src/parts.tsx`, the one file that
/// draws a box.
const NOT_A_DRAFT =
  'Use a part of src/parts.tsx that takes a draft (SayBox, SendForm, DraftArea, DraftInput): words typed in a box that holds them in its own state are lost when the box goes away.';
const BOX_PATHS = [{ name: '@primer/react', importNames: ['Textarea', 'TextInput'], message: NOT_A_DRAFT }];
const BOX_ELEMENTS = [{ selector: 'JSXOpeningElement[name.name=/^(input|textarea)$/]', message: NOT_A_DRAFT }];

/// The setting of `no-restricted-imports` that refuses these paths.
const refuse = (...paths) => ['error', { paths: paths.flat() }];

export default tseslint.config(
  { ignores: ['dist', 'node_modules'] },
  {
    files: ['**/*.{ts,tsx}'],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser, ...globals.node },
    },
    plugins: { 'react-hooks': reactHooks },
    rules: {
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'error',
      'no-restricted-imports': refuse(RAW_READ_PATHS, BOX_PATHS),
      'no-restricted-syntax': ['error', ...BOX_ELEMENTS],
    },
  },
  {
    // The reader and the hooks over it, the module that reads, and the reads the owner asks for by opening or scrolling
    // (a round's panel, the architect's list of rounds), which no event makes. They draw no box.
    files: ['src/reader.ts', 'src/live.ts', 'src/http.ts', 'src/rounds.ts', 'src/pages/RoundView.tsx'],
    rules: { 'no-restricted-imports': refuse(BOX_PATHS) },
  },
  {
    // The one file that draws the boxes the owner types in, as parts that take a draft. It reads nothing.
    files: ['src/parts.tsx'],
    rules: { 'no-restricted-imports': refuse(RAW_READ_PATHS), 'no-restricted-syntax': 'off' },
  },
);
