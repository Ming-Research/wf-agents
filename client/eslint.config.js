// The page's lint: the recommended JavaScript and TypeScript rules and the rules of hooks, with no warning let
// through (`npm run lint` passes --max-warnings 0); and that what events keep current is read through the reader.
import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

/// The reads that are not kept current by events: `getJson` and `getBody` read at once, with no coalescing, no gap
/// between reads and nothing shared between the parts that show a path. Parts read through `useResource`, `useShared`
/// and `useResources`, which the reader (`src/reader.ts`) makes one read at a time for each path.
const RAW_READS = ['getJson', 'getBody'];
const NOT_RAW = 'Read through useResource, useShared or useResources: an event-driven read made by hand is not coalesced.';

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
      'no-restricted-imports': [
        'error',
        {
          paths: ['./http', '../http', '../../http'].map((name) => ({ name, importNames: RAW_READS, message: NOT_RAW })),
        },
      ],
    },
  },
  {
    // The reader and the hooks over it, the module that reads, and the reads the owner asks for by opening or scrolling
    // (a round's panel, the architect's list of rounds), which no event makes.
    files: ['src/reader.ts', 'src/live.ts', 'src/http.ts', 'src/rounds.ts', 'src/pages/RoundView.tsx'],
    rules: { 'no-restricted-imports': 'off' },
  },
);
